#![deny(warnings)]
#![feature(custom_test_frameworks)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


use ku::memory::{
    mmu::{PageTableEntry, PageTableFlags, PAGE_TABLE_ENTRY_COUNT},
    size::{Size, TiB},
};

use kernel::{
    log::debug,
    memory::{
        test_scaffolding::{
            find_unused_block,
            forbid_frame_leaks,
            page_allocator_block,
            LOWER_HALF_ROOT_LEVEL_ENTRY_COUNT,
        },
        Page,
        Virt,
        BASE_ADDRESS_SPACE,
        KERNEL_READ,
        USER_READ,
    },
    Subsystems,
};


mod gen_main;

gen_main!(Subsystems::PHYS_MEMORY | Subsystems::VIRT_MEMORY);


#[test_case]
fn sanity_check() {
    let _guard = forbid_frame_leaks();

    let page_allocator_block = page_allocator_block(&BASE_ADDRESS_SPACE.lock());

    debug!(?page_allocator_block);

    assert!(page_allocator_block.size() > 100 * TiB);
    assert!(page_allocator_block.size() < 128 * TiB);
}


#[test_case]
fn some_used_entries() {
    let _guard = forbid_frame_leaks();

    let tests: [&[usize]; 6] = [&[], &[0], &[2], &[31], &[31, 254], &[0, 2, 31, 254]];

    for used_entries in tests.iter() {
        let mut page_table_root = [PageTableEntry::default(); PAGE_TABLE_ENTRY_COUNT];
        for used_entry in used_entries.iter() {
            for half in 0..=1 {
                page_table_root[used_entry + half * PAGE_TABLE_ENTRY_COUNT / 2]
                    .set_flags(PageTableFlags::PRESENT);
            }
        }

        let unused_block =
            find_unused_block(&page_table_root, 0..LOWER_HALF_ROOT_LEVEL_ENTRY_COUNT);

        debug!(?used_entries, ?unused_block);

        let unused_block = unused_block.unwrap();

        assert!(unused_block.size() > 100 * TiB);

        if used_entries.is_empty() {
            assert!(unused_block.size() == 128 * TiB);
        } else {
            assert!(unused_block.size() < 128 * TiB);
        }
    }
}


#[test_case]
fn allocate_block() {
    let _guard = forbid_frame_leaks();

    let mut address_space = BASE_ADDRESS_SPACE.lock();

    for size in (0..=3 * Page::SIZE).filter(|x| (x + 3) % Page::SIZE < 6) {
        let free_page_count = page_allocator_block(&address_space).count();
        let allocated = address_space.allocate(size, USER_READ).unwrap();
        let allocated_page_count = free_page_count - page_allocator_block(&address_space).count();

        debug!(requested = %Size::new::<Virt>(size), %allocated, allocated_page_count);

        assert!(
            allocated.size() >= size,
            "allocated less than the requested size",
        );
        assert!(
            allocated.size() - size < Page::SIZE,
            "allocated excessive memory",
        );
        assert_eq!(allocated.count(), allocated_page_count, "lost some pages");
    }
}


#[test_case]
fn allocate_page() {
    let _guard = forbid_frame_leaks();

    let page = BASE_ADDRESS_SPACE
        .lock()
        .allocate(Page::SIZE, KERNEL_READ)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();

    debug!(?page);

    assert_ne!(page.index(), 0);
}


#[test_case]
fn allocate_two_pages() {
    let _guard = forbid_frame_leaks();

    let mut address_space = BASE_ADDRESS_SPACE.lock();

    let pages = [
        address_space
            .allocate(Page::SIZE, KERNEL_READ)
            .unwrap()
            .into_iter()
            .next()
            .unwrap(),
        address_space
            .allocate(Page::SIZE, KERNEL_READ)
            .unwrap()
            .into_iter()
            .next()
            .unwrap(),
    ];

    debug!(?pages);

    assert_ne!(pages[0], pages[1]);
}
