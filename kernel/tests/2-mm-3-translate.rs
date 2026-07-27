#![feature(custom_test_frameworks)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


use kernel::{
    error::Error::{NoFrame, NoPage, Unimplemented},
    log::debug,
    memory::{
        mmu::{
            PageTableFlags,
            FULL_ACCESS,
            KERNEL_RW,
            PAGE_OFFSET_BITS,
            PAGE_TABLE_INDEX_BITS,
            PAGE_TABLE_INDEX_MASK,
            PAGE_TABLE_ROOT_LEVEL,
            USER,
            USER_RW,
        },
        test_scaffolding::{
            duplicate,
            forbid_frame_leaks,
            mapping,
            page_directory,
            phys2virt,
            phys2virt_map,
            Mapping,
        },
        AddressSpace,
        Frame,
        Virt,
        BASE_ADDRESS_SPACE,
        FRAME_ALLOCATOR,
    },
    Subsystems,
};


mod gen_main;

gen_main!(Subsystems::PHYS_MEMORY | Subsystems::VIRT_MEMORY);


#[test_case]
fn t1_translate() {
    let _guard = forbid_frame_leaks();

    let mut variable = 314159265;
    let write_ptr: *mut u64 = &mut variable;

    let virt = Virt::from_ref(&variable);

    let mut address_space = BASE_ADDRESS_SPACE.lock();
    let mapping = mapping(&mut address_space);

    let pte = mapping.translate(virt).unwrap();

    debug!(?pte);

    let frame = pte.frame().unwrap();
    let expected_flags = PageTableFlags::PRESENT |
        PageTableFlags::WRITABLE |
        PageTableFlags::ACCESSED |
        PageTableFlags::DIRTY;

    assert_eq!(pte.flags(), expected_flags);
    assert_ne!(frame, Frame::from_index(0).unwrap());

    let phys2virt = phys2virt(mapping);

    let page_offset_mask = (1 << PAGE_OFFSET_BITS) - 1;
    let page_offset = virt.into_usize() & page_offset_mask;
    let page_virt = phys2virt_map(phys2virt, frame.address());
    let alternative_virt = (page_virt + page_offset).unwrap();
    let read_ptr: *const u64 = alternative_virt.try_into_ptr().unwrap();

    debug!(?read_ptr, ?write_ptr);
    assert_ne!(read_ptr, write_ptr);

    const ITERATIONS: u64 = 5;

    for write_value in 0..ITERATIONS {
        let read_value = unsafe {
            write_ptr.write_volatile(write_value);
            read_ptr.read_volatile()
        };
        debug!(write_value, read_value, variable);
        assert_eq!(read_value, write_value);
        assert_eq!(read_value, variable);
    }
}


#[test_case]
fn t2_map_intermediate() {
    let mut frame_allocator = FRAME_ALLOCATOR.lock();
    let start_free_frames = frame_allocator.count();

    let mut address_space = BASE_ADDRESS_SPACE.lock();
    let mapping = mapping(&mut address_space);

    let virt = Virt::new(0xFFFF_FFFF_FFFF_FFFF).unwrap();
    let mut page_mapping = mapping.path(virt);
    page_mapping
        .map_intermediate(&mut frame_allocator, PageTableFlags::empty())
        .unwrap();
    let pte = page_mapping.get().unwrap();

    debug!(?pte);

    let end_free_frames = frame_allocator.count();
    assert!(start_free_frames > end_free_frames);

    check_intermediate_flags(mapping, virt, PageTableFlags::PRESENT);

    for flags in [KERNEL_RW, USER_RW] {
        mapping.path(virt).map_intermediate(&mut frame_allocator, flags).unwrap();
        assert!(frame_allocator.count() == end_free_frames);
        check_intermediate_flags(mapping, virt, flags);
    }
}


#[test_case]
fn t3_no_excessive_intermediate_flags() {
    let mut frame_allocator = FRAME_ALLOCATOR.lock();
    let start_free_frames = frame_allocator.count();

    let mut address_space = BASE_ADDRESS_SPACE.lock();
    let mapping = mapping(&mut address_space);

    let virt = Virt::new(0xFFFF_8FFF_FFFF_FFFF).unwrap();
    let flags = PageTableFlags::all().difference(FULL_ACCESS);
    let pte = mapping.path(virt).map_intermediate(&mut frame_allocator, flags).unwrap();

    debug!(?pte);

    let end_free_frames = frame_allocator.count();
    assert!(start_free_frames > end_free_frames);

    check_intermediate_flags(mapping, virt, PageTableFlags::PRESENT);
}


#[test_case]
fn t4_huge_page() {
    let mut address_space = BASE_ADDRESS_SPACE.lock();
    let mapping = mapping(&mut address_space);

    let phys2virt = phys2virt(mapping);
    let result = mapping
        .path(phys2virt.address())
        .map_intermediate(&mut FRAME_ALLOCATOR.lock(), USER);

    assert_eq!(result, Err(Unimplemented));
}


#[test_case]
fn t5_duplicate_drop() {
    let start_free_frames = FRAME_ALLOCATOR.lock().count();

    let mut address_space = BASE_ADDRESS_SPACE.lock();

    let a = Virt::new(0xFFFF_9FFF_FFFF_FFFF).unwrap();
    let b = Virt::new(0xFFFF_AFFF_FFFF_FFFF).unwrap();

    {
        let original_mapping = mapping(&mut address_space);
        original_mapping
            .path(a)
            .map_intermediate(&mut FRAME_ALLOCATOR.lock(), USER)
            .unwrap();
        check_intermediate_flags(original_mapping, a, USER);
    }

    let free_frames = map_in_duplicate(&mut address_space, false, true, a);
    assert!(start_free_frames > free_frames);

    {
        let original_mapping = mapping(&mut address_space);
        let pte = original_mapping.translate(b);
        assert_eq!(pte, Err(NoPage));
    }

    let end_free_frames = map_in_duplicate(&mut address_space, true, false, b);
    assert!(free_frames > end_free_frames);

    fn map_in_duplicate(
        address_space: &mut AddressSpace,
        allocate_intermediate: bool,
        mapped_in_original: bool,
        virt: Virt,
    ) -> usize {
        let _guard = forbid_frame_leaks();

        let mut address_space_copy = duplicate(address_space).unwrap();
        let mut mapping_copy = mapping(&mut address_space_copy);
        let mut original_mapping = mapping(address_space);

        {
            let mut frame_allocator = FRAME_ALLOCATOR.lock();
            let start_free_frames = frame_allocator.count();
            mapping_copy.path(virt).map_intermediate(&mut frame_allocator, USER).unwrap();
            let end_free_frames = frame_allocator.count();
            assert!(allocate_intermediate || end_free_frames == start_free_frames);
        }

        check_intermediate_flags(&mut mapping_copy, virt, USER);
        if mapped_in_original {
            check_intermediate_flags(&mut original_mapping, virt, USER);
        }

        let free_frames = FRAME_ALLOCATOR.lock().count();

        free_frames
    }
}


#[test_case]
fn t6_no_page() {
    let mut address_space = BASE_ADDRESS_SPACE.lock();
    let mapping = mapping(&mut address_space);

    let virt = Virt::new(0xFFFF_FFFF_FFDF_FFFF).unwrap();
    let pte = mapping.translate(virt);

    assert_eq!(pte, Err(NoPage));
}


#[test_case]
fn t9_no_frame() {
    let mut address_space = BASE_ADDRESS_SPACE.lock();
    let mapping = mapping(&mut address_space);

    let mut frame_allocator = FRAME_ALLOCATOR.lock();
    let frame_count = frame_allocator.count();
    let mut real_frame_count = 0;

    while frame_allocator.allocate().is_ok() {
        real_frame_count += 1;
    }

    assert!(real_frame_count <= frame_count);

    let virt = Virt::new(0xFFFF_FFFF_FFAF_FFFF).unwrap();
    let pte = mapping
        .path(virt)
        .map_intermediate(&mut frame_allocator, PageTableFlags::empty());
    assert_eq!(pte, Err(NoFrame));
}


fn check_intermediate_flags(mapping: &mut Mapping, virt: Virt, expected_flags: PageTableFlags) {
    let page_directory_index_shift =
        PAGE_TABLE_ROOT_LEVEL * PAGE_TABLE_INDEX_BITS + PAGE_OFFSET_BITS;
    let page_directory_index =
        (virt.into_usize() >> page_directory_index_shift) & PAGE_TABLE_INDEX_MASK;
    let intermediate_flags = page_directory(mapping)[page_directory_index].flags();
    assert_eq!(
        intermediate_flags, expected_flags,
        "intermediate page table flags are incorrect",
    );

    let leaf_flags: PageTableFlags = PageTableFlags::empty();
    let pte = mapping.translate(virt).unwrap();
    assert_eq!(
        pte.flags(),
        leaf_flags,
        "leaf page table flags are incorrect",
    );
}
