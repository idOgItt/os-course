#![feature(custom_test_frameworks)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


use ku::memory::size::{Size, SizeOf, TiB};

use kernel::{
    log::debug,
    memory::{
        test_scaffolding::{forbid_frame_leaks, page_allocator_block},
        Page,
        Virt,
        BASE_ADDRESS_SPACE,
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
fn allocate_block() {
    let _guard = forbid_frame_leaks();

    let mut address_space = BASE_ADDRESS_SPACE.lock();

    for size in (0..=3 * Page::SIZE).filter(|x| (x + 3) % Page::SIZE < 6) {
        let free_page_count = page_allocator_block(&address_space).count();
        let allocated = address_space.allocate(size).unwrap();
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
        .allocate(Page::SIZE)
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
        address_space.allocate(Page::SIZE).unwrap().into_iter().next().unwrap(),
        address_space.allocate(Page::SIZE).unwrap().into_iter().next().unwrap(),
    ];

    debug!(?pages);

    assert_ne!(pages[0], pages[1]);
}


#[test_case]
fn allocate_block() {
    let _guard = forbid_frame_leaks();

    let requested_size = 10 * Page::SIZE - Page::SIZE / 2;

    let block = BASE_ADDRESS_SPACE.lock().allocate(requested_size).unwrap();

    debug!(requested_size = %Size::new::<Virt>(requested_size), ?block);

    assert!(block.size() >= requested_size);
    assert!(block.count() <= Page::count_up(requested_size));
}
