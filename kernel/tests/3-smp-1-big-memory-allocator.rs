#![deny(warnings)]
#![feature(allocator_api)]
#![feature(custom_test_frameworks)]
#![feature(ptr_as_uninit)]
#![feature(slice_ptr_get)]
#![feature(strict_provenance)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


extern crate alloc;

use alloc::{
    alloc::{Allocator, Global, Layout},
    boxed::Box,
    collections::BTreeMap,
    vec::Vec,
};
use core::{cmp, mem};

use static_assertions::const_assert;

use ku::{
    allocator::{BigAllocator, DryAllocator, Initialize},
    error::{Error::NoPage, Result},
};

use kernel::{
    allocator,
    log::debug,
    memory::{
        mmu::{PageTableEntry, PageTableFlags, USER_RW},
        size::{KiB, MiB, Size},
        test_scaffolding::{forbid_frame_leaks, mapping},
        AddressSpace,
        Block,
        Page,
        Virt,
        BASE_ADDRESS_SPACE,
        FRAME_ALLOCATOR,
    },
    Subsystems,
};


mod gen_main;

gen_main!(Subsystems::MEMORY);


#[test_case]
fn basic() {
    memory_allocator_basic();
}


#[test_case]
fn alignment() {
    memory_allocator_alignment();
}


#[test_case]
fn grow_and_shrink_unmap_old_block() {
    let _guard = forbid_frame_leaks();

    let mut address_space = BASE_ADDRESS_SPACE.lock();

    let small_layout = Layout::from_size_align(4 * Page::SIZE, Page::SIZE).unwrap();
    let small_allocation = address_space
        .allocator(USER_RW)
        .dry_allocate(small_layout, Initialize::Garbage)
        .unwrap();
    let small_block = Block::from_slice(unsafe { small_allocation.as_uninit_slice() }).enclosing();

    let big_layout = Layout::from_size_align(8 * Page::SIZE, Page::SIZE).unwrap();
    let big_allocation = unsafe {
        address_space
            .allocator(USER_RW)
            .dry_grow(
                small_allocation.as_non_null_ptr(),
                small_layout,
                big_layout,
                Initialize::Garbage,
            )
            .unwrap()
    };
    let big_block = Block::from_slice(unsafe { big_allocation.as_uninit_slice() }).enclosing();

    debug!(%small_block, %big_block);

    let unmapped_pages = difference_should_be_unmapped(&mut address_space, small_block, big_block);
    debug!(unmapped_pages);
    assert!(unmapped_pages <= small_block.count());

    let medium_layout = Layout::from_size_align(6 * Page::SIZE, Page::SIZE).unwrap();
    let medium_allocation = unsafe {
        address_space
            .allocator(USER_RW)
            .dry_shrink(big_allocation.as_non_null_ptr(), big_layout, medium_layout)
            .unwrap()
    };
    let medium_block =
        Block::from_slice(unsafe { medium_allocation.as_uninit_slice() }).enclosing();

    debug!(%big_block, %medium_block);

    let unmapped_pages = difference_should_be_unmapped(&mut address_space, big_block, medium_block);
    debug!(unmapped_pages);
    assert!(unmapped_pages >= big_block.count() - medium_block.count());

    unsafe {
        address_space
            .allocator(USER_RW)
            .dry_deallocate(medium_allocation.as_non_null_ptr(), medium_layout);
    }

    fn difference_should_be_unmapped(
        address_space: &mut AddressSpace,
        new: Block<Page>,
        old: Block<Page>,
    ) -> usize {
        let new_minus_old_left =
            Block::<Page>::from_index(new.start(), cmp::min(new.end(), old.start()));
        let new_minus_old_right =
            Block::<Page>::from_index(cmp::max(new.start(), old.end()), new.end());

        let mut unmapped_pages = 0;

        for block in [new_minus_old_left, new_minus_old_right] {
            if let Ok(block) = block {
                for page in block {
                    assert_eq!(unsafe { address_space.unmap_page(page) }, Err(NoPage));
                }
                unmapped_pages += block.count();
            }
        }

        unmapped_pages
    }
}


#[test_case]
fn shrink_is_not_a_noop() {
    let mut address_space = BASE_ADDRESS_SPACE.lock();

    let old_layout = Layout::from_size_align(4 * Page::SIZE, Page::SIZE).unwrap();
    let old_allocation = address_space
        .allocator(USER_RW)
        .dry_allocate(old_layout, Initialize::Garbage)
        .unwrap();
    let old_block = Block::from_slice(unsafe { old_allocation.as_uninit_slice() });

    let new_layout = Layout::from_size_align(3 * Page::SIZE, Page::SIZE).unwrap();
    let new_allocation = unsafe {
        address_space
            .allocator(USER_RW)
            .dry_shrink(old_allocation.as_non_null_ptr(), old_layout, new_layout)
            .unwrap()
    };
    let new_block = Block::from_slice(unsafe { new_allocation.as_uninit_slice() });

    assert_ne!(old_block.count(), new_block.count());

    debug!(%old_block, %new_block, no_remap_on_shrink = old_block.contains_block(new_block));

    unsafe {
        address_space
            .allocator(USER_RW)
            .dry_deallocate(new_allocation.as_non_null_ptr(), new_layout);
    }
}


#[test_case]
fn frame_references_on_copy_mapping() {
    let mut address_space = BASE_ADDRESS_SPACE.lock();
    let layout = Layout::from_size_align(3 * Page::SIZE, Page::SIZE).unwrap();

    let mut old_block = Block::default();
    let mut new_block = Block::default();

    for block in [&mut old_block, &mut new_block] {
        *block = address_space.allocator(USER_RW).reserve(layout).unwrap();
        check_mapped(&mut address_space, *block, false);
    }

    unsafe {
        address_space.allocator(USER_RW).map(old_block, USER_RW).unwrap();
    }

    check_mapped(&mut address_space, old_block, true);
    check_mapped(&mut address_space, new_block, false);
    check_reference_count(&mut address_space, old_block, 1);

    unsafe {
        address_space
            .allocator(USER_RW)
            .copy_mapping(old_block, new_block, None)
            .unwrap();
    }

    check_mapped(&mut address_space, old_block, true);
    check_equal(&mut address_space, old_block, new_block);
    check_reference_count(&mut address_space, old_block, 2);

    unsafe {
        address_space.allocator(USER_RW).unmap(old_block).unwrap();
    }

    check_mapped(&mut address_space, old_block, false);
    check_mapped(&mut address_space, new_block, true);
    check_reference_count(&mut address_space, new_block, 1);

    unsafe {
        address_space.allocator(USER_RW).unmap(new_block).unwrap();
    }

    for block in [old_block, new_block] {
        check_mapped(&mut address_space, block, false);

        unsafe {
            address_space.allocator(USER_RW).unreserve(block).unwrap();
        }
    }

    fn check_equal(address_space: &mut AddressSpace, a: Block<Page>, b: Block<Page>) {
        debug!(%a, %b, "blocks should be equal");

        for (a_page, b_page) in a.into_iter().zip(b) {
            let a_pte = translate(address_space, a_page);
            let b_pte = translate(address_space, b_page);
            debug!(?a_pte, ?b_pte);
            assert_eq!(a_pte, b_pte);
        }
    }

    fn check_reference_count(
        address_space: &mut AddressSpace,
        block: Block<Page>,
        expected_reference_count: usize,
    ) {
        debug!(%block, expected_reference_count);

        for page in block {
            let translate_result = translate(address_space, page);
            if let Ok(pte) = translate_result {
                let reference_count =
                    FRAME_ALLOCATOR.lock().reference_count(pte.frame().unwrap()).unwrap();
                debug!(%page, ?pte, reference_count);
                assert_eq!(reference_count, expected_reference_count);
            } else {
                debug!(%page, ?translate_result);
                assert_eq!(
                    expected_reference_count, 0,
                    "intermediate page is not mapped?",
                );
            }
        }
    }

    fn check_mapped(address_space: &mut AddressSpace, block: Block<Page>, should_be_mapped: bool) {
        debug!(%block, should_be_mapped);

        for page in block {
            let translate_result = translate(address_space, page);
            if let Ok(pte) = translate_result {
                debug!(%page, ?pte);
                assert_eq!(pte.present(), should_be_mapped, "pte: {:?}", pte);
            } else {
                debug!(%page, ?translate_result);
                assert!(!should_be_mapped, "intermediate page is not mapped?");
            }
        }
    }
}


#[test_case]
fn grow_and_shrink() {
    memory_allocator_grow_and_shrink();
}


#[test_case]
fn paged_realloc_is_cheap() {
    #[repr(C, align(4096))]
    pub struct PagedType(u8);

    const_assert!(mem::align_of::<PagedType>() == Page::SIZE);
    const_assert!(mem::size_of::<PagedType>() == Page::SIZE);

    let mut vec = Vec::new();
    let mut prev_frames = Vec::new();

    for _ in 0..5 {
        vec.push(PagedType(0));
        while vec.len() < vec.capacity() {
            vec.push(PagedType(0));
        }

        prev_frames = check_frames(&vec, prev_frames);
    }

    while !vec.is_empty() {
        vec.pop().unwrap();
        vec.shrink_to_fit();

        prev_frames = check_frames(&vec, prev_frames);
    }

    assert!(prev_frames.is_empty());
}


#[test_case]
fn stress() {
    let values = 10_000;
    let max_fragmentation_loss = |values| cmp::max(8 * KiB * values, 16 * MiB);
    memory_allocator_stress(values, max_fragmentation_loss);
}


fn check_frames<T>(vec: &Vec<T>, prev_frames: Vec<PageTableEntry>) -> Vec<PageTableEntry> {
    let block = Block::from_slice(vec.as_slice()).enclosing();
    let mut frames = Vec::with_capacity(block.count());
    let mut address_space = BASE_ADDRESS_SPACE.lock();
    for page in block {
        let mut pte = translate(&mut address_space, page).unwrap();
        pte.set_flags(pte.flags() & !(PageTableFlags::ACCESSED | PageTableFlags::DIRTY));
        frames.push(pte)
    }
    debug!(%block, ?frames);

    for pte in &frames {
        let reference_count = FRAME_ALLOCATOR.lock().reference_count(pte.frame().unwrap()).unwrap();
        assert_eq!(
            reference_count, 1,
            "after a reallocation the frames should not be shared",
        );
    }

    let len = cmp::min(frames.len(), prev_frames.len());

    assert_eq!(
        frames[..len],
        prev_frames[..len],
        "the paged allocator should reuse existing frames on reallocations",
    );

    frames
}


fn translate(address_space: &mut AddressSpace, page: Page) -> Result<PageTableEntry> {
    mapping(address_space).translate(page.address()).map(|pte| pte.clone())
}


macro_rules! my_assert {
    ($cond:expr$(,)?) => {{
        assert!($cond);
    }};
}


include!("../../tests/memory_allocator.rs");
