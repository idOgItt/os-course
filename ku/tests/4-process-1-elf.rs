#![deny(warnings)]
#![feature(allocator_api)]
#![feature(gen_blocks)]
#![feature(int_roundings)]
#![feature(let_chains)]


use std::{alloc::Layout, cmp, fs, ops::Range, panic};

use anyhow::{bail, Context};
use duplicate::duplicate_item;
use rand::{distributions::Alphanumeric, rngs::SmallRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use xmas_elf::program::{Flags, ProgramHeader64, FLAG_R, FLAG_W, FLAG_X};

use ku::{
    allocator::BigAllocator,
    error::{
        Error::{InvalidArgument, NoPage, Overflow, PermissionDenied},
        Result,
    },
    log::{debug, info, trace},
    memory::{mmu::PageTableFlags, size, Block, Page, Virt, KERNEL_READ, USER, USER_READ},
    process::test_scaffolding::{
        combine,
        combine_flags,
        program_header_to_elf_range,
        ElfRange,
        Loader,
        PageRange,
    },
};


mod log;
mod mmap_fallback;


const SEED: u64 = 314159265;


#[test]
fn t00_flags() {
    log::init();

    for (ph_flags, flags) in [
        (FLAG_R, PageTableFlags::NO_EXECUTE),
        (
            FLAG_W,
            PageTableFlags::NO_EXECUTE | PageTableFlags::WRITABLE,
        ),
        (FLAG_X, PageTableFlags::default()),
        (
            FLAG_R | FLAG_W,
            PageTableFlags::NO_EXECUTE | PageTableFlags::WRITABLE,
        ),
        (FLAG_R | FLAG_X, PageTableFlags::default()),
    ] {
        assert_eq!(
            PageTableFlags::try_from(Flags(ph_flags)),
            Ok(flags | PageTableFlags::PRESENT),
        );
    }

    assert_eq!(
        PageTableFlags::try_from(Flags(FLAG_W | FLAG_X)),
        Err(PermissionDenied),
    );

    let mut program_header = program_header(0, 20, 0, 20);

    for ph_flags in generate_ph_flags() {
        program_header.flags = ph_flags;
        if let Ok(expected_flags) = PageTableFlags::try_from(program_header.flags) {
            let elf_range = program_header_to_elf_range(&program_header).unwrap();
            info!(%program_header.flags, %elf_range.flags, %expected_flags);
            assert_eq!(expected_flags, elf_range.flags);
        } else {
            let elf_range = program_header_to_elf_range(&program_header);
            info!(?elf_range, %ph_flags);
            assert_eq!(elf_range, Err(PermissionDenied));
        }
    }
}


#[test]
fn t01_ranges() {
    log::init();

    for start in 0..5 {
        for size in 0..5 {
            let memory_size = cmp::max(size, 1);
            let program_header = program_header(start, memory_size, start, size);
            let elf_range = program_header_to_elf_range(&program_header).unwrap();
            info!(?elf_range, ?program_header);
            assert_eq!(elf_range.memory.start(), start);
            assert_eq!(elf_range.memory.end(), start + memory_size);
            assert_eq!(elf_range.file_range, start..(start + size));
        }
    }
}


#[test]
fn t02_range_errors() {
    log::init();

    for (program_header, error) in [
        (
            program_header(0, Virt::higher_half().into_usize(), 0, 1),
            Overflow,
        ),
        (
            program_header(Virt::higher_half().into_usize(), usize::MAX / 2, 0, 1),
            Overflow,
        ),
        (program_header(usize::MAX / 2, 1, 0, 1), InvalidArgument),
        (program_header(0, 20, usize::MAX - 10, 20), Overflow),
        (program_header(0, 20, 0, 21), Overflow),
    ] {
        let elf_range = program_header_to_elf_range(&program_header);
        info!(?elf_range, ?program_header);
        assert_eq!(elf_range, Err(error));
    }
}


#[test]
fn t03_out_of_bounds() {
    log::init();

    const DATA: [u8; 3] = [1, 2, 3];

    let mut file = [0; DATA.len()];
    file.copy_from_slice(&DATA);
    let bad_file_range = 1..DATA.len() + 1;

    let mut target = [0; DATA.len()];

    let program_header = program_header_block(Block::from_slice_mut(&mut target), &bad_file_range);

    let elf_range = program_header_to_elf_range(&program_header).unwrap();
    info!(%elf_range, ?program_header);

    assert_eq!(elf_range.copy_to_memory(&file), Err(Overflow));
}


#[test]
fn t04_file_offset() {
    log::init();

    const DATA: [u8; 3] = [1, 2, 3];
    const OFFSET: usize = 7;

    let mut file = [0; OFFSET + DATA.len()];
    let file_range = OFFSET..OFFSET + DATA.len();
    file[file_range.clone()].copy_from_slice(&DATA);

    let mut target = [0; DATA.len()];

    let program_header = program_header_block(Block::from_slice_mut(&mut target), &file_range);

    let elf_range = program_header_to_elf_range(&program_header).unwrap();
    info!(%elf_range, ?program_header);

    elf_range.copy_to_memory(&file).unwrap();

    assert_eq!(&file[file_range], &target);
}


#[test]
fn t05_validate_order() {
    log::init();

    let a = Block::from_index(1, 2).unwrap();
    let b = Block::from_index(2, 3).unwrap();
    let c = Block::from_index(0, 2).unwrap();
    let d = Block::from_index(1, 3).unwrap();
    let e = Block::from_index(0, 3).unwrap();

    for (curr, next) in [(b, a), (a, c), (c, a), (a, d), (d, a), (a, e), (e, a)] {
        let curr = ElfRange::new(curr, USER_READ, 0..0);
        let next = ElfRange::new(next, USER_READ, 0..0);

        info!(%curr, %next);

        assert_eq!(combine(curr, next), Err(InvalidArgument));
    }
}


#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CombineTestCase {
    curr: ElfRange,
    next: ElfRange,
}


#[test]
fn t06_0_rerun_last_failed_test() {
    log::init();

    if let Err(error) = read_file(TEST_CASE_PATH)
        .and_then(|test_case| deserialize(&test_case))
        .and_then(|test_case| rerun(test_case))
    {
        trace!(
            ?error,
            file = TEST_CASE_PATH,
            "could not read the last failed test case",
        );
    }

    fn read_file(file: &str) -> anyhow::Result<String> {
        fs::read_to_string(file).with_context(|| format!("failed to read file {:?}", file))
    }

    fn rerun(test_case: CombineTestCase) -> anyhow::Result<()> {
        info!(?test_case, "rerunning the last failed test case");

        if panic::catch_unwind(|| validate_combine(test_case.curr, test_case.next)).is_err() {
            std::process::exit(1);
        }

        Ok(())
    }
}


#[test]
fn t06_1_stress_combine() {
    log::init();

    for curr in generate_blocks(0, 0..=2, -2..=2) {
        assert!(!curr.is_empty());
        for next in generate_blocks(curr.end(), 0..=2, -2..=2) {
            assert!(!next.is_empty());
            for curr_flags in generate_flags() {
                for next_flags in generate_flags() {
                    let curr = ElfRange::new(curr, curr_flags, 0..0);
                    let next = ElfRange::new(next, next_flags, 0..0);

                    let subtest_result = panic::catch_unwind(|| {
                        validate_combine(curr.clone(), next.clone());
                    });

                    if let Err(err) = subtest_result {
                        let test_case = CombineTestCase {
                            curr: curr.clone(),
                            next,
                        };

                        let serialized = serde_json::to_string_pretty(&test_case).unwrap();

                        if let Err(error) = validate_serialization(&test_case, &serialized)
                            .and_then(|_| write_file(TEST_CASE_PATH, serialized))
                        {
                            info!(?error, "could not write the failed test input");
                        } else {
                            info!(
                                file = TEST_CASE_PATH,
                                "saved the failed test case to the file",
                            );
                        }

                        panic::resume_unwind(err);
                    }
                }
            }
        }
    }

    fn write_file(file: &str, string: String) -> anyhow::Result<()> {
        fs::write(file, string).with_context(|| format!("failed to write file {:?}", file))
    }

    fn validate_serialization(test_case: &CombineTestCase, serialized: &str) -> anyhow::Result<()> {
        let deserialized = deserialize(serialized)?;
        if &deserialized != test_case {
            bail!(
                "serialization error: test_case = {:?}, deserialized = {:?}",
                test_case,
                deserialized,
            )
        } else {
            Ok(())
        }
    }
}


fn deserialize(test_case: &str) -> anyhow::Result<CombineTestCase> {
    serde_json::from_str::<CombineTestCase>(&test_case)
        .with_context(|| format!("failed to deserialize {:?}", test_case))
}


fn validate_combine(curr: ElfRange, next: ElfRange) {
    let (curr_minus_next, boundary, updated_next) = combine(curr.clone(), next.clone()).unwrap();

    trace!(%curr, %next, ?curr_minus_next, ?boundary, %updated_next, "check combine");

    assert!(
        !updated_next.memory.is_empty(),
        "it is better to avoid corner cases arising from empty ranges",
    );

    for validate in [
        validate_output_range,
        validate_output_page_count,
        validate_updated_next,
        validate_curr_minus_next_and_boundary_presence,
        validate_curr_minus_next,
        validate_boundary,
    ] {
        validate(&curr, &next, &curr_minus_next, &boundary, &updated_next);
    }

    fn validate_output_range(
        curr: &ElfRange,
        next: &ElfRange,
        curr_minus_next: &Option<PageRange>,
        boundary: &Option<PageRange>,
        updated_next: &ElfRange,
    ) {
        let result_start = if let Some(curr_minus_next) = curr_minus_next {
            curr_minus_next.start_address()
        } else if let Some(boundary) = boundary {
            boundary.start_address()
        } else {
            updated_next.start_address()
        };

        assert_eq!(
            Page::containing(curr.start_address()).address(),
            Page::containing(result_start).address(),
        );

        assert_eq!(next.end_address(), updated_next.end_address());
    }

    fn validate_output_page_count(
        curr: &ElfRange,
        next: &ElfRange,
        curr_minus_next: &Option<PageRange>,
        boundary: &Option<PageRange>,
        updated_next: &ElfRange,
    ) {
        let curr_pages = curr.memory.enclosing();
        let next_pages = next.memory.enclosing();
        let updated_next_pages = updated_next.memory.enclosing();

        let page_count = curr_pages.count() + next_pages.count() -
            if curr_pages.is_disjoint(next_pages) {
                0
            } else {
                1
            };

        let result_page_count = curr_minus_next.unwrap_or_default().memory.count() +
            boundary.unwrap_or_default().memory.count() +
            updated_next_pages.count();

        assert_eq!(page_count, result_page_count);
    }

    fn validate_curr_minus_next_and_boundary_presence(
        curr: &ElfRange,
        next: &ElfRange,
        curr_minus_next: &Option<PageRange>,
        boundary: &Option<PageRange>,
        updated_next: &ElfRange,
    ) {
        let curr_pages = curr.memory.enclosing();
        let next_pages = next.memory.enclosing();

        let page_adjacent =
            curr_pages.is_adjacent(next_pages) || !curr_pages.is_disjoint(next_pages);
        let can_merge = curr.flags == next.flags && page_adjacent;
        let different_pages = Page::containing(curr.start_address()) !=
            Page::containing((next.end_address().unwrap() - 1).unwrap());

        if !page_adjacent {
            assert!(boundary.is_none());
            assert_eq!(updated_next, next);
        }

        if !can_merge && different_pages {
            assert!(curr_minus_next.is_some() || boundary.is_some());
        }

        let curr_and_next_share_a_page = !curr_pages.is_disjoint(next_pages);
        if curr_and_next_share_a_page {
            assert_eq!(curr_pages.intersection(next_pages).count(), 1);
            if !can_merge {
                assert_eq!(curr_minus_next.is_some(), curr_pages.count() > 1);
            }
            if curr_pages.contains_block(next_pages) {
                assert!(boundary.is_none());
            } else {
                assert!(can_merge || boundary.is_some());
            }
        } else {
            assert!(boundary.is_none());
        }
    }

    fn validate_curr_minus_next(
        curr: &ElfRange,
        next: &ElfRange,
        curr_minus_next: &Option<PageRange>,
        boundary: &Option<PageRange>,
        updated_next: &ElfRange,
    ) {
        let curr_pages = curr.memory.enclosing();
        let updated_next_pages = updated_next.memory.enclosing();

        if let Some(curr_minus_next) = curr_minus_next {
            assert!(!curr_minus_next.memory.is_empty());
            assert!(curr_minus_next.memory < updated_next_pages);
            assert!(curr_pages.contains_block(curr_minus_next.memory));
            assert_eq!(curr.flags, curr_minus_next.flags);
            if curr_pages == curr_minus_next.memory {
                assert!(boundary.is_none());
                assert_eq!(next, updated_next);
            }
        }
    }

    fn validate_boundary(
        curr: &ElfRange,
        next: &ElfRange,
        curr_minus_next: &Option<PageRange>,
        boundary: &Option<PageRange>,
        updated_next: &ElfRange,
    ) {
        let curr_pages = curr.memory.enclosing();
        let next_pages = next.memory.enclosing();

        if let Some(boundary) = boundary {
            assert_eq!(boundary.memory.count(), 1);
            if let Some(curr_minus_next) = curr_minus_next {
                assert_eq!(
                    curr_minus_next.end_address().unwrap(),
                    boundary.start_address(),
                );
            }
            assert_eq!(
                boundary.end_address().unwrap(),
                updated_next.start_address(),
            );
            assert!(curr_pages.contains_block(boundary.memory));
            assert!(next_pages.contains_block(boundary.memory));
            validate_that_new_flags_include_old_flags(curr.flags, boundary.flags);
            validate_that_new_flags_include_old_flags(next.flags, boundary.flags);
        }
    }

    fn validate_updated_next(
        curr: &ElfRange,
        next: &ElfRange,
        curr_minus_next: &Option<PageRange>,
        boundary: &Option<PageRange>,
        updated_next: &ElfRange,
    ) {
        let curr_pages = curr.memory.enclosing();
        let updated_next_pages = updated_next.memory.enclosing();

        if curr_minus_next.is_none() && boundary.is_none() {
            assert!(updated_next_pages.contains_block(curr_pages));
            assert!(updated_next.memory.contains_block(next.memory));
        }

        let curr_intersects_with_updated_next = !updated_next.memory.is_disjoint(curr_pages.into());
        if curr_intersects_with_updated_next {
            validate_that_new_flags_include_old_flags(curr.flags, updated_next.flags);
            validate_that_new_flags_include_old_flags(next.flags, updated_next.flags);
        } else {
            assert_eq!(next.flags, updated_next.flags);
        }
    }

    fn validate_that_new_flags_include_old_flags(subset: PageTableFlags, superset: PageTableFlags) {
        assert!(
            (superset ^ PageTableFlags::NO_EXECUTE).contains(subset ^ PageTableFlags::NO_EXECUTE),
        );
    }
}


#[test]
fn t07_initial_flags() {
    let mut allocator = DummyAllocator::new(1, PageTableFlags::default());
    let offset = allocator.offset();

    let file = [];

    let curr = None;
    let next = ElfRange::new(
        Block::new(offset, (offset + Page::SIZE).unwrap()).unwrap(),
        PageTableFlags::default(),
        0..0,
    );

    let mut loader = Loader::new(&mut allocator, &file);
    let expected_initial_flags = loader.initial_flags();

    loader.extend_mapping(&curr, &next).unwrap();
    let initial_flags = allocator.pages[0].flags;

    debug!(%initial_flags);
    assert!(initial_flags.contains(PageTableFlags::WRITABLE));
    assert_eq!(initial_flags, expected_initial_flags);
}


#[test]
fn t08_finalize_mapping() {
    log::init();

    let mut rng = SmallRng::seed_from_u64(SEED);

    let block_flags: Vec<_> = generate_flags().collect();

    for allocator_flags in [
        KERNEL_READ,
        USER,
        PageTableFlags::NO_CACHE,
        PageTableFlags::ACCESSED | PageTableFlags::DIRTY,
        PageTableFlags::AVAILABLE,
    ] {
        let page_count = 32;
        let min_block_pages = 1;
        let max_block_pages = 4;
        let max_gap_pages = 4;
        let file_size = max_block_pages * Page::SIZE;

        let mut allocator = DummyAllocator::new(page_count, allocator_flags);
        let offset = allocator.offset();
        let offset_page = allocator.offset_page();

        let file: Vec<_> = (&mut rng).sample_iter(Alphanumeric).take(file_size).collect();

        let initial_flags = Loader::new(&mut allocator, &file).initial_flags();

        let mut prev_end = 0;
        let mut expected = Vec::new();

        while prev_end < page_count {
            let start = cmp::min(
                rng.gen_range(prev_end..=prev_end + max_gap_pages),
                page_count,
            );

            while prev_end < start {
                expected.push(DummyPage {
                    flags: PageTableFlags::default(),
                    reserved: false,
                });
                prev_end += 1;
            }

            if start >= page_count {
                break;
            }

            let end = cmp::min(
                rng.gen_range(start + min_block_pages..=start + max_block_pages),
                page_count,
            );
            let page_range = PageRange::new(
                Block::from_index(offset_page + start, offset_page + end).unwrap(),
                block_flags[rng.gen_range(0..block_flags.len())],
            );

            while prev_end < end {
                expected.push(DummyPage {
                    flags: page_range.flags | allocator.flags(),
                    reserved: true,
                });
                prev_end += 1;
            }

            let size = page_range.memory.size();

            allocator.reserve_fixed(page_range.memory).unwrap();
            unsafe {
                allocator.map(page_range.memory, initial_flags).unwrap();
                page_range
                    .memory
                    .try_into_mut_slice::<u8>()
                    .unwrap()
                    .copy_from_slice(&file[..size]);
            }

            debug!(%page_range);

            let mut loader = Loader::new(&mut allocator, &file);
            loader.finalize_mapping(page_range.clone()).unwrap();

            let actual_memory = unsafe { page_range.memory.try_into_mut_slice::<u8>().unwrap() };
            validate_memory(
                &actual_memory,
                &file[..size],
                &format!("actual_memory[..{size}] (at offset {offset})"),
                &format!("file[..{size}]"),
            );
        }

        assert_eq!(allocator.pages, expected);
    }
}


#[test]
fn t09_extend_mapping() {
    log::init();

    let mut rng = SmallRng::seed_from_u64(SEED);

    let block_flags: Vec<_> = generate_flags().collect();

    for allocator_flags in [
        KERNEL_READ,
        USER,
        PageTableFlags::NO_CACHE,
        PageTableFlags::ACCESSED | PageTableFlags::DIRTY,
        PageTableFlags::AVAILABLE,
    ] {
        let page_count = 32;
        let end = page_count * Page::SIZE;
        let min_block_len = 1;
        let max_block_len = 4 * Page::SIZE;
        let max_gap_len = 4 * Page::SIZE;

        let mut allocator = DummyAllocator::new(page_count, allocator_flags);
        let offset = allocator.offset();

        let file = [];

        let mut loader = Loader::new(&mut allocator, &file);

        let mut curr_end = 0;
        let mut expected = Vec::new();
        let mut curr = None;

        while curr_end < page_count * Page::SIZE {
            let next_start = cmp::min(rng.gen_range(curr_end..=curr_end + max_gap_len), end);

            while curr_end.div_ceil(Page::SIZE) < next_start.div_floor(Page::SIZE) {
                expected.push(DummyPage {
                    flags: PageTableFlags::default(),
                    reserved: false,
                });
                curr_end += Page::SIZE;
            }

            if next_start >= page_count * Page::SIZE {
                break;
            }

            let next_end = cmp::min(
                rng.gen_range(next_start + min_block_len..=next_start + max_block_len),
                end,
            );
            let next = ElfRange::new(
                Block::new((offset + next_start).unwrap(), (offset + next_end).unwrap()).unwrap(),
                block_flags[rng.gen_range(0..block_flags.len())],
                0..0,
            );

            while curr_end < next_end {
                expected.push(DummyPage {
                    flags: loader.initial_flags(),
                    reserved: true,
                });
                curr_end += Page::SIZE;
            }

            debug!(?curr, %next);

            loader.extend_mapping(&curr, &next).unwrap();

            curr = Some(next);
        }

        assert_eq!(allocator.pages, expected);

        allocator.validate_no_garbage();
    }
}


#[test]
fn t10_load_program_header() {
    log::init();

    let mut rng = SmallRng::seed_from_u64(SEED);

    let block_flags: Vec<_> = generate_flags().collect();

    for (page_count, max_block_len, max_gap_len) in
        [(16, 8, 8), (1024, 8 * Page::SIZE, 8 * Page::SIZE)]
    {
        for iteration in 0..16 {
            info!(page_count, max_block_len, max_gap_len, iteration);

            for allocator_flags in [
                KERNEL_READ,
                USER,
                PageTableFlags::NO_CACHE,
                PageTableFlags::ACCESSED | PageTableFlags::DIRTY,
                PageTableFlags::AVAILABLE,
            ] {
                let allocator_flags = allocator_flags | PageTableFlags::PRESENT;
                let end = page_count * Page::SIZE;
                let min_block_len = 1;
                let file_size = 16 * Page::SIZE;

                let mut allocator = DummyAllocator::new(page_count, allocator_flags);
                let offset = allocator.offset();

                let file: Vec<_> = (&mut rng).sample_iter(Alphanumeric).take(file_size).collect();

                let mut loader = Loader::new(&mut allocator, &file);

                let mut curr_end = 0;
                let mut expected_pages = Vec::new();
                let mut expected_memory = vec![0u8; end];
                let mut curr = None;

                while curr_end < page_count * Page::SIZE {
                    let next_start =
                        cmp::min(rng.gen_range(curr_end..=curr_end + max_gap_len), end);

                    while curr_end.div_ceil(Page::SIZE) < next_start.div_floor(Page::SIZE) {
                        expected_pages.push(DummyPage {
                            flags: PageTableFlags::default(),
                            reserved: false,
                        });
                        curr_end += Page::SIZE;
                    }

                    if next_start >= page_count * Page::SIZE {
                        break;
                    }

                    let next_end = cmp::min(
                        rng.gen_range(next_start + min_block_len..=next_start + max_block_len),
                        end,
                    );
                    let memory =
                        Block::new((offset + next_start).unwrap(), (offset + next_end).unwrap())
                            .unwrap();
                    let file_size = rng.gen_range(0..=memory.size());
                    let file_start = rng.gen_range(0..file.len() - file_size);
                    let file_end = file_start + file_size;
                    let next = ElfRange::new(
                        memory,
                        block_flags[rng.gen_range(0..block_flags.len())],
                        file_start..file_end,
                    );

                    expected_memory[next_start..next_start + file_size]
                        .copy_from_slice(&file[file_start..file_end]);

                    if curr_end.div_ceil(Page::SIZE) > next_start.div_floor(Page::SIZE) {
                        let boundary = expected_pages.len() - 1;
                        expected_pages[boundary].flags =
                            combine_flags(expected_pages[boundary].flags, next.flags);
                    }
                    while curr_end.div_ceil(Page::SIZE) < next_end.div_ceil(Page::SIZE) {
                        expected_pages.push(DummyPage {
                            flags: next.flags | allocator_flags,
                            reserved: true,
                        });
                        curr_end += Page::SIZE;
                    }

                    trace!(?curr, %next);

                    let updated_next = loader.load_program_header(curr, next).unwrap();

                    let actual_memory = unsafe { memory.try_into_slice::<u8>().unwrap() };
                    validate_memory(
                        &actual_memory[..file_size],
                        &file[file_start..file_end],
                        &format!("actual_memory[{next_start}..{next_end}] (at offset {offset})"),
                        &format!("file[{file_start}..{file_end}]"),
                    );
                    assert!(actual_memory[file_size..].iter().all(|&x| x == 0));

                    curr_end = next_end;
                    curr = Some(updated_next);
                }

                if let Some(curr) = curr {
                    loader.finalize_mapping(curr.into()).unwrap();
                }

                assert_eq!(allocator.pages, expected_pages);
                allocator.validate_allocated_memory_state(&expected_memory);
                allocator.validate_no_garbage();
                allocator.validate_no_unused_pages();
            }
        }
    }
}


fn validate_memory(actual: &[u8], expected: &[u8], actual_name: &str, expected_name: &str) {
    const STEP: usize = 8;

    assert_eq!(
        actual.len(),
        expected.len(),
        "lengths do not match (actual {}, expected {})",
        actual_name,
        expected_name,
    );

    let len = actual.len();

    for start in (0..len).step_by(STEP) {
        let end = (start + STEP).min(len);

        assert_eq!(
            actual[start..end],
            expected[start..end],
            "contents at [{}, {}) do not match (actual {}, expected {})",
            start,
            end,
            actual_name,
            expected_name,
        );
    }
}


fn generate_blocks(
    from: usize,
    pages: impl Clone + IntoIterator<Item = isize>,
    offsets: impl Clone + IntoIterator<Item = isize>,
) -> impl Iterator<Item = Block<Virt>> {
    let page_size: isize = Page::SIZE.try_into().unwrap();

    gen move {
        for start_page in pages.clone().into_iter() {
            for start_offset in offsets.clone().into_iter() {
                let start = isize::try_from(from).unwrap() + start_page * page_size + start_offset;
                if let Ok(start) = start.try_into() &&
                    from <= start
                {
                    for end_page in pages.clone().into_iter() {
                        for end_offset in offsets.clone().into_iter() {
                            let end = (start_page + end_page) * page_size + end_offset;
                            if let Ok(end) = end.try_into() &&
                                start < end
                            {
                                yield Block::from_index(start, end).unwrap()
                            }
                        }
                    }
                }
            }
        }
    }
    .into_iter()
}


fn generate_flags() -> impl Iterator<Item = PageTableFlags> {
    gen {
        for executable in [PageTableFlags::NO_EXECUTE, PageTableFlags::default()] {
            for writeble in [PageTableFlags::default(), PageTableFlags::WRITABLE] {
                yield executable | writeble | PageTableFlags::PRESENT
            }
        }
    }
    .into_iter()
}


fn generate_ph_flags() -> impl Iterator<Item = Flags> {
    gen {
        for ph_flags in 0..8 {
            yield Flags(ph_flags)
        }
    }
    .into_iter()
}


fn program_header(
    virt: usize,
    memory_size: usize,
    file_offset: usize,
    file_size: usize,
) -> ProgramHeader64 {
    let mut ph = ProgramHeader64::default();

    ph.virtual_addr = size::into_u64(virt);
    ph.mem_size = size::into_u64(memory_size);
    ph.offset = size::into_u64(file_offset);
    ph.file_size = size::into_u64(file_size);

    ph
}


fn program_header_block(memory: Block<Virt>, file: &Range<usize>) -> ProgramHeader64 {
    program_header(
        memory.start_address().into_usize(),
        memory.size(),
        file.start,
        file.clone().count(),
    )
}


#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DummyPage {
    flags: PageTableFlags,
    reserved: bool,
}


struct DummyAllocator {
    flags: PageTableFlags,
    memory: Vec<u8>,
    offset_index: usize,
    pages: Vec<DummyPage>,
}


impl DummyAllocator {
    fn new(page_count: usize, flags: PageTableFlags) -> Self {
        let mut allocator = Self {
            flags,
            memory: Vec::new(),
            offset_index: 0,
            pages: Vec::new(),
        };

        allocator.memory.resize((page_count + 1) * Page::SIZE, Self::GARBAGE);
        allocator.pages.resize(page_count, DummyPage::default());

        allocator.offset_index =
            (Page::containing(Virt::from_ref(&allocator.memory[Page::SIZE - 1])).address() -
                Virt::from_ref(&allocator.memory[0]))
            .unwrap();

        allocator
    }


    #[allow(clippy::needless_arbitrary_self_type)]
    #[duplicate_item(
        getter reference(x);
        [get] [&x];
        [get_mut] [&mut x];
    )]
    fn getter(self: reference([Self]), range: Range<usize>) -> reference([[u8]]) {
        let start = range.start + self.offset_index;
        let end = range.end + self.offset_index;
        self.memory.getter(start..end).unwrap()
    }


    fn offset(&self) -> Virt {
        Virt::from_ref(&self.memory[self.offset_index])
    }


    fn offset_page(&self) -> usize {
        Page::containing(self.offset()).index()
    }


    fn reserved_count(&self, block: Block<Page>) -> usize {
        let offset_page = self.offset_page();
        (block.start()..block.end())
            .filter(|&i| self.pages[i - offset_page].reserved)
            .count()
    }


    fn validate_no_garbage(&self) {
        for (i, dummy_page) in self.pages.iter().enumerate() {
            if dummy_page.reserved {
                let page = Page::from_index(self.offset_page() + i).unwrap();
                let start = i * Page::SIZE;
                let end = start + Page::SIZE;
                let slice = self.get(start..end);

                let has_garbage = slice.iter().find(|&&x| x == Self::GARBAGE).is_some();

                if has_garbage {
                    info!(%page, "found some garbage (chunks of {:#2X})", Self::GARBAGE);
                    for chunk in
                        slice.chunk_by(|&a, &b| (a == Self::GARBAGE) == (b == Self::GARBAGE))
                    {
                        let block = Block::from_slice(chunk);
                        info!(%block, "    {:#2X} x {}", chunk[0], chunk.len());
                    }
                }

                assert!(!has_garbage, "page {page} is not zeroed properly");
            }
        }
    }


    fn validate_no_unused_pages(&self) {
        for (i, dummy_page) in self.pages.iter().enumerate() {
            if dummy_page.reserved {
                let page = Page::from_index(self.offset_page() + i).unwrap();

                assert!(
                    dummy_page.flags.contains(PageTableFlags::PRESENT),
                    "page {page} is reserved but is not unused",
                );
            }
        }
    }


    fn validate_allocated_memory_state(&self, expected: &[u8]) {
        for page in 0..self.pages.len() {
            if !self.pages[page].reserved {
                continue;
            }

            let page_start = page * Page::SIZE;
            let page_end = (page + 1) * Page::SIZE;

            validate_memory(
                self.get(page_start..page_end),
                &expected[page_start..page_end],
                &format!("actual_memory[{page_start}..{page_end}]"),
                &format!(
                    "expected_memory[{}..{}] (at offset {})",
                    page_start,
                    page_end,
                    self.offset(),
                ),
            );
        }
    }


    const GARBAGE: u8 = 0xFF;
}


unsafe impl BigAllocator for DummyAllocator {
    fn flags(&self) -> PageTableFlags {
        self.flags
    }


    fn set_flags(&mut self, flags: PageTableFlags) -> Result<()> {
        self.flags = flags;
        Ok(())
    }


    fn reserve(&mut self, _layout: Layout) -> Result<Block<Page>> {
        unimplemented!();
    }


    fn reserve_fixed(&mut self, block: Block<Page>) -> Result<()> {
        assert!(block.end() - self.offset_page() <= self.pages.len());

        if self.reserved_count(block) == 0 {
            let offset_page = self.offset_page();
            for i in block.start()..block.end() {
                self.pages[i - offset_page].reserved = true;
            }
            Ok(())
        } else {
            Err(NoPage)
        }
    }


    unsafe fn unreserve(&mut self, _block: Block<Page>) -> Result<()> {
        unimplemented!();
    }


    unsafe fn rereserve(&mut self, _old_block: Block<Page>, _sub_block: Block<Page>) -> Result<()> {
        unimplemented!();
    }


    unsafe fn map(&mut self, block: Block<Page>, flags: PageTableFlags) -> Result<()> {
        assert_eq!(
            self.reserved_count(block),
            block.count(),
            "do not forget to reserve_fixed() a page block before mapping",
        );

        assert!(
            flags.contains(PageTableFlags::PRESENT),
            "map() with non-present flags makes no sense",
        );

        let offset_page = self.offset_page();
        for i in block.start()..block.end() {
            self.pages[i - offset_page].flags = flags;

            let page = Page::from_index(i - offset_page).unwrap();
            self.get_mut(page.address().into_usize()..(page + 1).unwrap().address().into_usize())
                .fill(Self::GARBAGE);
        }

        Ok(())
    }


    unsafe fn unmap(&mut self, _block: Block<Page>) -> Result<()> {
        unimplemented!();
    }


    unsafe fn copy_mapping(
        &mut self,
        old_block: Block<Page>,
        new_block: Block<Page>,
        flags: Option<PageTableFlags>,
    ) -> Result<()> {
        assert_eq!(old_block, new_block);
        assert!(flags.is_some());

        assert_eq!(
            self.reserved_count(new_block),
            new_block.count(),
            "do not forget to reserve_fixed() a page block before mapping",
        );

        for i in new_block.start()..new_block.end() {
            let i = i - self.offset_page();
            self.pages[i].flags = flags.unwrap();
        }

        Ok(())
    }
}


const TEST_CASE_PATH: &str = "last_failed_t06_stress_combine.json";
