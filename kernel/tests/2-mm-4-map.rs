#![feature(custom_test_frameworks)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


use core::{cmp, mem};

use ku::memory::size::MiB;

use kernel::{
    error::Error::PermissionDenied,
    log::debug,
    memory::{
        mmu::{PageTableEntry, PageTableFlags},
        test_scaffolding::{forbid_frame_leaks, map_page, mapping, total_frames},
        Block,
        Frame,
        Page,
        Virt,
        BASE_ADDRESS_SPACE,
        FRAME_ALLOCATOR,
        KERNEL_READ,
        KERNEL_RW,
        USER_READ,
    },
    Subsystems,
};


mod gen_main;

gen_main!(Subsystems::PHYS_MEMORY | Subsystems::VIRT_MEMORY);


#[test_case]
fn map_slice() {
    let start_free_frames = FRAME_ALLOCATOR.lock().count();

    let total_frames = total_frames();
    let len = cmp::min(total_frames * Frame::SIZE / 2, 16 * MiB) / mem::size_of::<usize>();
    let slice = BASE_ADDRESS_SPACE.lock().map_slice(len, KERNEL_RW, usize::default).unwrap();
    let slice_frames = mem::size_of_val(slice).div_ceil(Frame::SIZE);

    let free_frames = FRAME_ALLOCATOR.lock().count();
    assert!(free_frames + slice_frames <= start_free_frames);

    debug!(slice = ?Block::from_slice(slice));
    debug!(slice_frames, total_frames);

    for (i, element) in slice.iter_mut().enumerate() {
        *element = i;
    }

    for (i, element) in slice.iter().enumerate() {
        assert_eq!(*element, i);
    }

    unsafe {
        BASE_ADDRESS_SPACE.lock().unmap_slice(slice).unwrap();
    }

    let end_free_frames = FRAME_ALLOCATOR.lock().count();
    assert!(free_frames + slice_frames <= end_free_frames);
}


#[test_case]
fn map_readable() {
    let page = Page::containing(Virt::new(0x0000_6000_0000_0000).unwrap());

    let frame = unsafe {
        map_page(
            &mut BASE_ADDRESS_SPACE.lock(),
            page,
            PageTableFlags::empty(),
        )
        .unwrap()
    };
    debug!(?frame);

    unsafe {
        Block::from_element(page).unwrap().try_into_slice::<u8>().unwrap().iter().max();
    }
}


#[test_case]
fn map_writable() {
    let page = Page::containing(Virt::new(0x0000_6000_1000_0000).unwrap());

    let frame = unsafe {
        map_page(
            &mut BASE_ADDRESS_SPACE.lock(),
            page,
            PageTableFlags::WRITABLE,
        )
        .unwrap()
    };
    debug!(?frame);

    unsafe {
        Block::from_element(page).unwrap().try_into_mut_slice::<u8>().unwrap().fill(12);
    }
}


#[test_case]
fn map_twice() {
    let virt = Virt::new(0x0000_4000_0000_0000).unwrap();
    let page = Page::containing(virt);

    let old_frame = unsafe {
        map_page(
            &mut BASE_ADDRESS_SPACE.lock(),
            page,
            PageTableFlags::empty(),
        )
        .unwrap()
    };

    let pte = translate(virt);
    assert_eq!(KERNEL_READ, pte.flags());

    let _guard = forbid_frame_leaks();

    let new_frame = unsafe { map_page(&mut BASE_ADDRESS_SPACE.lock(), page, KERNEL_RW).unwrap() };

    let pte = translate(virt);
    assert_eq!(KERNEL_RW, pte.flags());

    let hacked_frame = unsafe { map_page(&mut BASE_ADDRESS_SPACE.lock(), page, USER_READ) };
    assert_eq!(hacked_frame, Err(PermissionDenied));

    debug!(?old_frame, ?new_frame);
    assert_ne!(old_frame, new_frame);

    fn translate(virt: Virt) -> PageTableEntry {
        let mut address_space = BASE_ADDRESS_SPACE.lock();
        *mapping(&mut address_space).translate(virt).unwrap()
    }
}
