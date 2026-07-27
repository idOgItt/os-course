#![feature(custom_test_frameworks)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


use ku::{error::Result, memory::size::MiB};

use kernel::{
    error::Error::NoFrame,
    log::debug,
    memory::{
        mmu::{
            self,
            PageTableEntry,
            PageTableFlags,
            FULL_ACCESS,
            PAGE_TABLE_ENTRY_COUNT,
            PAGE_TABLE_ROOT_LEVEL,
        },
        test_scaffolding::{
            duplicate,
            forbid_frame_leaks,
            map_page,
            mapping,
            new_address_space,
            page_directory,
            phys2virt,
            switch_to,
            total_frames,
            unmap_page,
        },
        AddressSpace,
        Frame,
        Page,
        Virt,
        BASE_ADDRESS_SPACE,
        FRAME_ALLOCATOR,
        KERNEL_RW,
        USER_READ,
    },
    Subsystems,
};


mod gen_main;

gen_main!(Subsystems::MEMORY);


#[test_case]
fn sanity_check() {
    let _guard = forbid_frame_leaks();

    let frame_allocator = FRAME_ALLOCATOR.lock();
    let free_frames = frame_allocator.count();

    let qemu_memory_frames = 128 * MiB / Frame::SIZE;
    let min_free_frames = qemu_memory_frames - 16 * MiB / Frame::SIZE;

    debug!(free_frames, min_free_frames, qemu_memory_frames);

    assert!(free_frames > min_free_frames);
    assert!(free_frames < qemu_memory_frames);
}


#[test_case]
fn basic_frame_allocator_functions() {
    let _guard = forbid_frame_leaks();

    let mut frame_allocator = FRAME_ALLOCATOR.lock();
    let start_free_frames = frame_allocator.count();
    let total_frames = total_frames();

    let frames = [
        frame_allocator.allocate().unwrap(),
        frame_allocator.allocate().unwrap(),
    ];

    debug!(?frames, total_frames);
    assert!(frames[0] != frames[1]);
    assert!(frame_allocator.count() == start_free_frames - 2);
    for frame in frames {
        assert!(
            frame.index() < total_frames,
            "allocated a frame outside of the physical memory of the current machine",
        );
    }

    frame_allocator.deallocate(frames[0]);
    assert!(frame_allocator.count() == start_free_frames - 1);

    let reallocate_last_freed_frame = frame_allocator.allocate().unwrap();

    debug!(?reallocate_last_freed_frame);
    assert!(reallocate_last_freed_frame == frames[0]);
    assert!(frame_allocator.count() == start_free_frames - 2);

    frame_allocator.reference(frames[1]);
    assert!(frame_allocator.count() == start_free_frames - 2);

    frame_allocator.deallocate(frames[1]);
    assert!(frame_allocator.count() == start_free_frames - 2);

    frame_allocator.deallocate(frames[1]);
    assert!(frame_allocator.count() == start_free_frames - 1);

    frame_allocator.deallocate(frames[0]);
}


#[test_case]
fn allocated_frames_are_not_used() {
    let _guard = forbid_frame_leaks();

    let max_frames_for_intermediates: usize = PAGE_TABLE_ROOT_LEVEL.try_into().unwrap();
    let poison = 0xDEAD_BEEF_DEAD_BEEF_u64;

    let mut address_space = BASE_ADDRESS_SPACE.lock();
    let start_free_frames = FRAME_ALLOCATOR.lock().count();
    let total_frames = total_frames();

    let frames = address_space.map_slice(start_free_frames, KERNEL_RW, Frame::default).unwrap();

    let page_block = address_space.allocate(Page::SIZE).unwrap();
    let page = page_block.into_iter().next().unwrap();

    let start_free_frames = FRAME_ALLOCATOR.lock().count();
    debug!(start_free_frames, total_frames);

    let mut frames_poisoned = 0;
    let mut prev_frame = None;

    while let Ok(frame) = allocate_frame() {
        {
            let mut frame_allocator = FRAME_ALLOCATOR.lock();
            assert_eq!(frame_allocator.reference_count(frame), Ok(1));
            frame_allocator.reference(frame);
            assert_eq!(frame_allocator.reference_count(frame), Ok(2));
        }

        assert!(
            frame.index() < total_frames,
            "allocated a frame {frame} outside of the physical memory of the current machine with only {total_frames} frames total",
        );

        frames[frames_poisoned] = frame;

        if let Some(prev_frame) = prev_frame {
            assert_eq!(FRAME_ALLOCATOR.lock().reference_count(prev_frame), Ok(2));
        }
        unsafe {
            address_space.map_page_to_frame(page, frame, KERNEL_RW).unwrap();
            mmu::flush(page);
        }
        if let Some(prev_frame) = prev_frame {
            assert_eq!(FRAME_ALLOCATOR.lock().reference_count(prev_frame), Ok(1));
        }
        prev_frame = Some(frame);

        let slice = unsafe { page_block.try_into_mut_slice().unwrap() };
        slice.fill(poison);

        if frames_poisoned % 1000 == 0 {
            debug!(
                frames_poisoned,
                %frame,
                poison = slice[frames_poisoned % slice.len()],
                "poisoning the free memory",
            );
        }

        frames_poisoned += 1;
    }

    let prev_frame = prev_frame.expect("at least one frame should be free");
    assert_eq!(FRAME_ALLOCATOR.lock().reference_count(prev_frame), Ok(2));
    unsafe {
        unmap_page(&mut address_space, page).unwrap();
    }
    assert_eq!(FRAME_ALLOCATOR.lock().reference_count(prev_frame), Ok(1));

    let end_free_frames = FRAME_ALLOCATOR.lock().count();
    debug!(start_free_frames, end_free_frames, frames_poisoned);
    assert_eq!(end_free_frames, 0);
    assert!(frames_poisoned >= start_free_frames - max_frames_for_intermediates);

    introsort::sort(&mut frames[..frames_poisoned]);
    for (a, b) in frames.iter().zip(&frames[1..frames_poisoned]) {
        assert_ne!(a, b, "allocated the same frame twice");
    }

    for &frame in &frames[..frames_poisoned] {
        let mut frame_allocator = FRAME_ALLOCATOR.lock();
        assert_eq!(frame_allocator.reference_count(frame), Ok(1));
        frame_allocator.deallocate(frame);
        assert_eq!(frame_allocator.reference_count(frame), Ok(0));
    }

    debug!(free_frames = FRAME_ALLOCATOR.lock().count());
    assert!(FRAME_ALLOCATOR.lock().count() >= start_free_frames - max_frames_for_intermediates);

    unsafe {
        address_space.unmap_slice(frames).unwrap();
    }

    fn allocate_frame() -> Result<Frame> {
        FRAME_ALLOCATOR.lock().allocate()
    }
}


#[test_case]
fn allocated_frames_are_unique() {
    let _guard = forbid_frame_leaks();

    let start_free_frames = FRAME_ALLOCATOR.lock().count();
    let total_frames = total_frames();
    let frames = BASE_ADDRESS_SPACE
        .lock()
        .map_slice(start_free_frames, KERNEL_RW, Frame::default)
        .unwrap();

    {
        let mut frame_allocator = FRAME_ALLOCATOR.lock();
        let free_frames = frame_allocator.count();
        debug!(free_frames, total_frames);

        for i in 0..free_frames {
            let frame = frame_allocator.allocate().unwrap();
            assert!(
                frame.index() < total_frames,
                "allocated a frame {frame} outside of the physical memory of the current machine with only {total_frames} frames total",
            );
            frames[i] = frame;
        }

        debug!(free_frames = frame_allocator.count());
        assert!(frame_allocator.count() == 0);
        assert_eq!(frame_allocator.allocate(), Err(NoFrame));

        introsort::sort(&mut frames[..free_frames]);
        for (a, b) in frames.iter().zip(&frames[1..free_frames]) {
            assert_ne!(a, b, "allocated the same frame twice");
        }

        for &frame in &frames[..free_frames] {
            frame_allocator.deallocate(frame);
        }

        debug!(free_frames = frame_allocator.count());
        assert!(frame_allocator.count() == free_frames);
    }

    unsafe {
        BASE_ADDRESS_SPACE.lock().unmap_slice(frames).unwrap();
    }
}


#[test_case]
fn shared_memory() {
    let _guard = forbid_frame_leaks();

    let frame = FRAME_ALLOCATOR.lock().allocate().expect("failed to allocate a frame");

    debug!(?frame);

    FRAME_ALLOCATOR.lock().reference(frame);

    let mut address_space = BASE_ADDRESS_SPACE.lock();

    let mut pages_iterator = address_space.allocate(2 * Page::SIZE).unwrap().into_iter();

    let pages = [
        pages_iterator.next().unwrap(),
        pages_iterator.next().unwrap(),
    ];

    debug!(?pages);

    unsafe {
        for page in pages {
            address_space
                .map_page_to_frame(page, frame, KERNEL_RW)
                .expect("failed to map a page frame");
        }
    }

    let read_ptr: *const u64 = pages[0].address().try_into_ptr().unwrap();
    let write_ptr: *mut u64 = pages[1].address().try_into_mut_ptr().unwrap();
    debug!(?write_ptr, ?read_ptr);
    assert!(read_ptr != write_ptr);

    const ITERATIONS: u64 = 3;

    for write_value in 0..ITERATIONS {
        let read_value = unsafe {
            write_ptr.write_volatile(write_value);
            read_ptr.read_volatile()
        };
        debug!(write_value, read_value);
        assert!(read_value == write_value);
    }

    for page in pages {
        unsafe {
            unmap_page(&mut address_space, page).unwrap();
        }
    }
}


#[test_case]
fn garbage_after_map_intermediate() {
    let mut frame_allocator = FRAME_ALLOCATOR.lock();
    let start_free_frames = frame_allocator.count();

    let mut address_space = BASE_ADDRESS_SPACE.lock();
    let mapping = mapping(&mut address_space);

    let virt = Virt::new(0x0000_4000_0000_0000).unwrap();
    let mut page_mapping = mapping.path(virt);
    page_mapping
        .map_intermediate(&mut frame_allocator, PageTableFlags::empty())
        .unwrap();
    let pte = page_mapping.get().unwrap();

    let expected_pte = PageTableEntry::default();

    debug!(?pte, ?expected_pte);
    assert_eq!(*pte, expected_pte);

    let end_free_frames = frame_allocator.count();

    assert_ne!(start_free_frames, end_free_frames);

    for neighbour in 1..PAGE_TABLE_ENTRY_COUNT {
        let neighbour_virt = (virt + neighbour * Page::SIZE).unwrap();
        let neighbour_pte = mapping.translate(neighbour_virt).unwrap();
        if *neighbour_pte != expected_pte {
            debug!(?neighbour_pte, ?expected_pte);
        }
        assert_eq!(*neighbour_pte, expected_pte);
    }
}


#[test_case]
fn duplicate_and_drop_mapping() {
    let _guard = forbid_frame_leaks();

    let variable =
        unsafe { BASE_ADDRESS_SPACE.lock().map_slice_zeroed::<usize>(1, KERNEL_RW).unwrap() };
    let virt = Virt::from_ref(&mut variable[0]);
    let frame = translate(&mut BASE_ADDRESS_SPACE.lock(), virt).frame().unwrap();

    let reference_count = FRAME_ALLOCATOR.lock().reference_count(frame).unwrap();
    debug!(%virt, %frame, %reference_count, "one address space");
    assert_eq!(reference_count, 1);

    let mut address_space_copy = duplicate(&BASE_ADDRESS_SPACE.lock()).unwrap();
    let mut address_space = BASE_ADDRESS_SPACE.lock();

    let reference_count = FRAME_ALLOCATOR.lock().reference_count(frame).unwrap();
    debug!(%virt, %frame, %reference_count, "two identical address spaces");
    assert_eq!(reference_count, 2);

    let ptes = [
        translate(&mut address_space, virt),
        translate(&mut address_space_copy, virt),
    ];
    let pte_addresses = [Virt::from_ref(ptes[0]), Virt::from_ref(ptes[1])];

    debug!(?ptes);
    debug!(?pte_addresses);

    assert_eq!(ptes[0], ptes[1]);
    assert_ne!(pte_addresses[0], pte_addresses[1]);

    drop(address_space_copy);

    let reference_count = FRAME_ALLOCATOR.lock().reference_count(frame).unwrap();
    debug!(%virt, %frame, %reference_count, "back to one address space");
    assert_eq!(reference_count, 1);

    unsafe {
        address_space.unmap_slice(variable).unwrap();
    }
}


#[test_case]
fn all_frames_are_freed_on_drop() {
    const PAGES_PER_ROOT_LEVEL_ENTRY: usize = PAGE_TABLE_ENTRY_COUNT.pow(PAGE_TABLE_ROOT_LEVEL);

    let _guard = forbid_frame_leaks();

    let page_directory_frame = FRAME_ALLOCATOR.lock().allocate().unwrap();
    let phys2virt = phys2virt(mapping(&mut BASE_ADDRESS_SPACE.lock()));
    let mut address_space = new_address_space(page_directory_frame, phys2virt);

    page_directory(mapping(&mut address_space)).fill(PageTableEntry::default());

    for (i, flags) in (0..=FULL_ACCESS.bits())
        .map(|flags| PageTableFlags::from_bits_truncate(flags))
        .filter(|flags| flags.contains(PageTableFlags::PRESENT))
        .enumerate()
    {
        let page = Page::from_index(i * PAGES_PER_ROOT_LEVEL_ENTRY).unwrap();
        unsafe {
            map_page(&mut address_space, page, flags).unwrap();
        }
        let frame = translate(&mut address_space, page.address()).frame().unwrap();
        debug!(%page, %frame, ?flags);
    }
}


#[test_case]
fn garbage_in_duplicate() {
    let virt = Virt::new(0x0000_3000_0000_0000).unwrap();
    let page = Page::containing(virt);

    unsafe {
        map_page(&mut BASE_ADDRESS_SPACE.lock(), page, KERNEL_RW).unwrap();
    }

    let _guard = forbid_frame_leaks();

    let mut address_space_copy = duplicate(&BASE_ADDRESS_SPACE.lock()).unwrap();
    let mut address_space = BASE_ADDRESS_SPACE.lock();

    let ptes = [
        translate(&mut address_space, virt),
        translate(&mut address_space_copy, virt),
    ];
    let pte_addresses = [Virt::from_ref(ptes[0]), Virt::from_ref(ptes[1])];

    debug!(?ptes);
    debug!(?pte_addresses);

    assert_eq!(ptes[0], ptes[1]);
    assert_ne!(pte_addresses[0], pte_addresses[1]);

    for neighbour in 1..PAGE_TABLE_ENTRY_COUNT {
        let neighbour_virt = (virt + neighbour * Page::SIZE).unwrap();
        let expected_neighbour_pte = translate(&mut address_space, neighbour_virt);
        let neighbour_pte = translate(&mut address_space_copy, neighbour_virt);
        if *neighbour_pte != *expected_neighbour_pte {
            debug!(?neighbour_pte, ?expected_neighbour_pte);
        }
        assert_eq!(*neighbour_pte, *expected_neighbour_pte);
    }

    drop(address_space_copy);
}


#[test_case]
fn duplicate_only_kernel() {
    let page = BASE_ADDRESS_SPACE
        .lock()
        .allocate(Page::SIZE)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let virt = page.address();

    debug!(?page);

    unsafe {
        map_page(&mut BASE_ADDRESS_SPACE.lock(), page, USER_READ).unwrap();
    }

    let mut address_space_copy = duplicate(&BASE_ADDRESS_SPACE.lock()).unwrap();
    let mut address_space = BASE_ADDRESS_SPACE.lock();

    let ptes = [
        translate(&mut address_space, virt),
        translate(&mut address_space_copy, virt),
    ];
    debug!(?ptes);
    assert!(ptes[0].present());
    assert!(!ptes[1].present(), "do not copy mappings to user pages");

    drop(address_space_copy);

    unsafe {
        unmap_page(&mut address_space, page).unwrap();
    }
}


#[test_case]
fn duplicate_works() {
    let _guard = forbid_frame_leaks();

    let variable =
        unsafe { BASE_ADDRESS_SPACE.lock().map_slice_zeroed::<usize>(1, KERNEL_RW).unwrap() };
    variable[0] = 0;
    debug!(
        variable = variable[0],
        "original address space initializes the variable",
    );

    let address_space_copy = duplicate(&BASE_ADDRESS_SPACE.lock()).unwrap();
    let mut address_space = BASE_ADDRESS_SPACE.lock();

    switch_to(&address_space_copy);

    debug!("inside the duplicate address space");
    variable[0] = 1;
    debug!(
        variable = variable[0],
        "duplicate address space modifies the variable",
    );

    switch_to(&address_space);

    drop(address_space_copy);

    debug!(
        variable = variable[0],
        "original address space reads the variable",
    );
    assert_eq!(variable[0], 1);

    unsafe {
        address_space.unmap_slice(variable).unwrap();
    }
}


#[test_case]
fn tlb_flush() {
    let _guard = forbid_frame_leaks();

    let old_mark = 1111111;
    let new_mark = 2222222;

    let old_frame = get_marked_frame(old_mark);
    let new_frame = get_marked_frame(new_mark);

    let mut address_space = BASE_ADDRESS_SPACE.lock();

    let page = address_space.allocate(Page::SIZE).unwrap().into_iter().next().unwrap();

    unsafe {
        address_space
            .map_page_to_frame(page, old_frame, KERNEL_RW)
            .expect("failed to map a page frame");
    }

    let current_mark = get_page_mark(page);
    debug!(
        %page,
        %old_frame,
        current_mark,
        old_mark,
        "the page is currently mapped to the old frame and should contain its old mark",
    );
    assert_eq!(current_mark, old_mark);

    unsafe {
        address_space
            .map_page_to_frame(page, new_frame, KERNEL_RW)
            .expect("failed to map a page frame");
    }

    let current_mark = get_page_mark(page);
    debug!(
        %page,
        %new_frame,
        current_mark,
        new_mark,
        "the page is currently mapped to the new frame and should contain its new mark",
    );
    assert_eq!(current_mark, new_mark);

    unsafe {
        unmap_page(&mut address_space, page).unwrap();
    }

    FRAME_ALLOCATOR.lock().deallocate(old_frame);
    FRAME_ALLOCATOR.lock().deallocate(new_frame);

    fn get_marked_frame(mark: u64) -> Frame {
        let frame = FRAME_ALLOCATOR.lock().allocate().expect("failed to allocate a frame");
        FRAME_ALLOCATOR.lock().reference(frame);
        FRAME_ALLOCATOR.lock().reference(frame);

        let mut address_space = BASE_ADDRESS_SPACE.lock();

        let page = address_space.allocate(Page::SIZE).unwrap().into_iter().next().unwrap();

        unsafe {
            address_space
                .map_page_to_frame(page, frame, KERNEL_RW)
                .expect("failed to map a page frame");
        }

        unsafe {
            page.address().try_into_mut_ptr::<u64>().unwrap().write_volatile(mark);
        }

        let frame_mark = get_page_mark(page);
        debug!(%frame, %page, frame_mark);
        assert_eq!(mark, get_page_mark(page));

        unsafe {
            unmap_page(&mut address_space, page).unwrap();
        }

        frame
    }

    fn get_page_mark(page: Page) -> u64 {
        unsafe { page.address().try_into_ptr::<u64>().unwrap().read_volatile() }
    }
}


fn translate(address_space: &mut AddressSpace, virt: Virt) -> &PageTableEntry {
    mapping(address_space).translate(virt).unwrap()
}
