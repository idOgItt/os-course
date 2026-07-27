#![deny(warnings)]
#![feature(custom_test_frameworks)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


use core::{cmp, mem};

use ku::memory::size::MiB;

use kernel::{
    error::{
        Error::{NoPage, PermissionDenied},
        Result,
    },
    log::debug,
    memory::{
        mmu::{PageTableEntry, PageTableFlags},
        test_scaffolding::{
            forbid_frame_leaks,
            frame_count,
            kernel_root_level_entries,
            map_page,
            map_page_to_frame,
            mapping,
            unmap_page,
            user_root_level_entries,
            PAGES_PER_ROOT_LEVEL_ENTRY,
        },
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
mod mm_helpers;


gen_main!(Subsystems::PHYS_MEMORY | Subsystems::VIRT_MEMORY);


#[test_case]
fn map_slice() {
    let _guard = forbid_frame_leaks();

    let start_free_frames = FRAME_ALLOCATOR.lock().count();

    let mut address_space = BASE_ADDRESS_SPACE.lock();
    let frame_count = frame_count();
    let len = cmp::min(frame_count * Frame::SIZE / 2, 16 * MiB) / mem::size_of::<usize>();
    let slice = address_space.map_slice(len, KERNEL_RW, usize::default).unwrap();
    let slice_frames = mem::size_of_val(slice).div_ceil(Frame::SIZE);

    let free_frames = FRAME_ALLOCATOR.lock().count();
    assert!(free_frames + slice_frames <= start_free_frames);

    debug!(slice = ?Block::from_slice(slice));
    debug!(slice_frames, frame_count);

    for (i, element) in slice.iter_mut().enumerate() {
        *element = i;
    }

    for (i, element) in slice.iter().enumerate() {
        assert_eq!(*element, i);
    }

    unsafe {
        address_space.unmap_slice(slice).unwrap();
    }

    let end_free_frames = FRAME_ALLOCATOR.lock().count();
    assert!(free_frames + slice_frames <= end_free_frames);
}


#[test_case]
fn map_readable() {
    let _guard = forbid_frame_leaks();

    let mut address_space = BASE_ADDRESS_SPACE.lock();

    let page = Page::containing(mm_helpers::unique_kernel_virt());
    let frame = unsafe { map_page(&mut address_space, page, PageTableFlags::empty()).unwrap() };
    debug!(%page, %frame);

    unsafe {
        Block::from_element(page).unwrap().try_into_slice::<u8>().unwrap().iter().max();

        unmap_page(&mut address_space, page).unwrap();
    }
}


fn check_map_writable(virt: Virt, remap: bool, map_neighbour: bool) {
    let _guard = forbid_frame_leaks();

    let mut address_space = BASE_ADDRESS_SPACE.lock();

    let page = Page::containing(virt);

    if map_neighbour {
        let neighbour_page = (page + 1).unwrap();

        unsafe {
            map_page(&mut address_space, neighbour_page, KERNEL_READ).unwrap();
            unmap_page(&mut address_space, neighbour_page).unwrap();
        }
    }

    if remap {
        unsafe {
            map_page(&mut address_space, page, KERNEL_READ).unwrap();
        }
    }

    let frame = unsafe { map_page(&mut address_space, page, KERNEL_RW).unwrap() };
    debug!(%page, %frame);

    unsafe {
        Block::from_element(page).unwrap().try_into_mut_slice::<u8>().unwrap().fill(12);

        unmap_page(&mut address_space, page).unwrap();
    }
}


#[test_case]
fn map_writable() {
    for remap in [false, true] {
        for map_neighbour in [false, true] {
            let virt = mm_helpers::unique_kernel_virt();
            check_map_writable(virt, remap, map_neighbour);
        }
    }
}


#[test_case]
fn map_twice() {
    let _guard = forbid_frame_leaks();

    let virt = mm_helpers::unique_kernel_virt();
    let page = Page::containing(virt);

    let old_frame = unsafe { map_page(&mut BASE_ADDRESS_SPACE.lock(), page, KERNEL_READ).unwrap() };

    let pte = translate(virt);
    assert_eq!(KERNEL_READ, pte.flags());

    let new_frame = unsafe { map_page(&mut BASE_ADDRESS_SPACE.lock(), page, KERNEL_RW).unwrap() };

    let pte = translate(virt);
    assert_eq!(KERNEL_RW, pte.flags());

    let hacked_frame = unsafe { map_page(&mut BASE_ADDRESS_SPACE.lock(), page, USER_READ) };
    assert_eq!(hacked_frame, Err(PermissionDenied));

    debug!(?old_frame, ?new_frame);
    assert_ne!(old_frame, new_frame);

    unsafe {
        unmap_page(&mut BASE_ADDRESS_SPACE.lock(), page).unwrap();
    }

    fn translate(virt: Virt) -> PageTableEntry {
        let mut address_space = BASE_ADDRESS_SPACE.lock();
        *mapping(&mut address_space).translate(virt).unwrap()
    }
}


#[test_case]
fn unmap_nonmapped_page() {
    let _guard = forbid_frame_leaks();

    let virt = mm_helpers::unique_kernel_virt();
    let page = Page::containing(virt);

    let address_space = &mut BASE_ADDRESS_SPACE.lock();

    unsafe {
        assert_eq!(unmap_page(address_space, page), Err(NoPage));

        map_page(address_space, page, PageTableFlags::empty()).unwrap();

        assert_eq!(unmap_page(address_space, page), Ok(()));
        assert_eq!(unmap_page(address_space, page), Err(NoPage));
    }
}


#[test_case]
fn kernel_and_user_are_separated() {
    let _guard = forbid_frame_leaks();

    let frame = FRAME_ALLOCATOR.lock().allocate().unwrap();
    let checks_per_root_level_entry = 16;

    for i in kernel_root_level_entries() {
        for j in 0..checks_per_root_level_entry {
            let page = Page::from_index(
                i * PAGES_PER_ROOT_LEVEL_ENTRY +
                    j * PAGES_PER_ROOT_LEVEL_ENTRY / checks_per_root_level_entry,
            )
            .unwrap();

            for map_function in [map_page_function, map_page_to_frame_function] {
                assert_eq!(
                    map_function(page, frame, USER_READ),
                    Err(PermissionDenied),
                    "kernel {page:?} is accessible for user space",
                );
            }
        }
    }

    for i in user_root_level_entries() {
        for j in 0..checks_per_root_level_entry {
            let page = Page::from_index(
                i * PAGES_PER_ROOT_LEVEL_ENTRY +
                    j * PAGES_PER_ROOT_LEVEL_ENTRY / checks_per_root_level_entry,
            )
            .unwrap();

            for map_function in [map_page_function, map_page_to_frame_function] {
                assert!(
                    map_function(page, frame, KERNEL_READ).is_err(),
                    "user space {page:?} is mappable with kernel flags",
                );
            }
        }
    }

    FRAME_ALLOCATOR.lock().deallocate(frame);

    fn map_page_function(page: Page, _frame: Frame, flags: PageTableFlags) -> Result<()> {
        unsafe { map_page(&mut BASE_ADDRESS_SPACE.lock(), page, flags).map(|_| ()) }
    }

    fn map_page_to_frame_function(page: Page, frame: Frame, flags: PageTableFlags) -> Result<()> {
        unsafe { map_page_to_frame(&mut BASE_ADDRESS_SPACE.lock(), page, frame, flags) }
    }
}
