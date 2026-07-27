#![deny(warnings)]
#![feature(custom_test_frameworks)]
#![feature(iter_is_partitioned)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


use itertools::Itertools;

use kernel::{
    error::Error::{NoFrame, NoPage, Unimplemented},
    log::debug,
    memory::{
        mmu::{
            PageTableEntry,
            PageTableFlags,
            FULL_ACCESS,
            KERNEL_RW,
            PAGE_OFFSET_BITS,
            PAGE_TABLE_INDEX_BITS,
            PAGE_TABLE_INDEX_MASK,
            PAGE_TABLE_LEAF_LEVEL,
            PAGE_TABLE_ROOT_LEVEL,
            USER,
            USER_RW,
        },
        size,
        test_scaffolding::{
            deepest_pte,
            duplicate,
            forbid_frame_leaks,
            mapping,
            nodes,
            page_table_root,
            phys2virt,
            switch_to,
            Mapping,
        },
        AddressSpace,
        Frame,
        Page,
        Phys,
        Virt,
        BASE_ADDRESS_SPACE,
        FRAME_ALLOCATOR,
    },
    Subsystems,
};


mod gen_main;
mod mm_helpers;


gen_main!(Subsystems::PHYS_MEMORY | Subsystems::VIRT_MEMORY);


fn path(address_space: &mut AddressSpace) {
    let mapping = mapping(address_space);

    let variable = 314159265;
    let present_virt = Virt::from_ref(&variable);
    let path = mapping.path(present_virt);
    let (level, pte) = deepest_pte(&path);
    debug!(%present_virt, %path, level);
    assert_eq!(level, PAGE_TABLE_LEAF_LEVEL);
    debug!(?pte);
    assert!(pte.present());
    assert!(path.get().is_ok());
    check_path(mapping, present_virt);

    let non_present_virt = mm_helpers::unique_user_virt();
    let path = mapping.path(non_present_virt);
    let (level, pte) = deepest_pte(&path);
    debug!(%non_present_virt, %path, level, ?pte);
    assert!(level > PAGE_TABLE_LEAF_LEVEL);
    assert_ne!(
        Virt::from_ref(pte),
        Page::containing(Virt::from_ref(pte)).address(),
        "nodes should be addresses of PageTableEntries, not addresses of PageTables",
    );
    assert!(!pte.present());
    assert_eq!(path.get(), Err(NoPage));
    check_path(mapping, non_present_virt);

    let huge_virt = phys2virt(mapping).map(Phys::default()).unwrap();
    let path = mapping.path(huge_virt);
    let (level, pte) = deepest_pte(&path);
    debug!(%huge_virt, %path, level, ?pte);
    assert_ne!(
        level, PAGE_TABLE_LEAF_LEVEL,
        "a Path should not descend deeper into a huge page",
    );
    assert!(pte.present());
    assert!(pte.flags().contains(PageTableFlags::HUGE_PAGE));
    assert_eq!(path.get(), Err(Unimplemented));
    check_path(mapping, huge_virt);
}


#[test_case]
fn t00_path() {
    let _guard = forbid_frame_leaks();

    path(&mut BASE_ADDRESS_SPACE.lock());
}


fn translate(address_space: &mut AddressSpace) {
    let mut variable = 314159265;
    let write_ptr: *mut u64 = &mut variable;

    let virt = Virt::from_ref(&variable);

    let mapping = mapping(address_space);

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
    let page_virt = phys2virt.map(frame.address()).unwrap();
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
fn t01_translate() {
    let _guard = forbid_frame_leaks();

    translate(&mut BASE_ADDRESS_SPACE.lock());
}


fn map_intermediate(virt: Virt) {
    let start_free_frames = FRAME_ALLOCATOR.lock().count();

    let mut address_space = BASE_ADDRESS_SPACE.lock();
    let mapping = mapping(&mut address_space);

    let mut path = mapping.path(virt);
    path.map_intermediate(PageTableFlags::empty()).unwrap();
    let pte = path.get().unwrap();

    debug!(?pte);

    let end_free_frames = FRAME_ALLOCATOR.lock().count();
    assert!(start_free_frames > end_free_frames);

    check_intermediate_flags(mapping, virt, PageTableFlags::PRESENT);

    for flags in [KERNEL_RW, USER_RW] {
        mapping.path(virt).map_intermediate(flags).unwrap();
        assert!(FRAME_ALLOCATOR.lock().count() == end_free_frames);
        check_intermediate_flags(mapping, virt, flags);
    }
}


fn do_not_loose_intermediate_flags(virt: Virt) {
    map_intermediate(virt, KERNEL_RW);
    check_intermediate(virt, KERNEL_RW);

    map_intermediate(virt, USER);
    check_intermediate(virt, USER_RW);

    fn check_intermediate(virt: Virt, flags: PageTableFlags) {
        let mut address_space = BASE_ADDRESS_SPACE.lock();
        let mapping = mapping(&mut address_space);

        check_intermediate_flags(mapping, virt, flags);
    }

    fn map_intermediate(virt: Virt, flags: PageTableFlags) {
        let mut address_space = BASE_ADDRESS_SPACE.lock();
        let mapping = mapping(&mut address_space);

        let mut path = mapping.path(virt);
        path.map_intermediate(flags).unwrap();
    }
}


#[test_case]
fn t02_map_intermediate() {
    map_intermediate(mm_helpers::unique_user_virt());
    do_not_loose_intermediate_flags(mm_helpers::unique_user_virt());
}


fn no_excessive_intermediate_flags(virt: Virt) {
    let start_free_frames = FRAME_ALLOCATOR.lock().count();

    let mut address_space = BASE_ADDRESS_SPACE.lock();
    let mapping = mapping(&mut address_space);

    let flags = PageTableFlags::all().difference(FULL_ACCESS);
    let pte = mapping.path(virt).map_intermediate(flags).unwrap();

    debug!(?pte);

    let end_free_frames = FRAME_ALLOCATOR.lock().count();
    assert!(start_free_frames > end_free_frames);

    check_intermediate_flags(mapping, virt, PageTableFlags::PRESENT);
}


#[test_case]
fn t03_no_excessive_intermediate_flags() {
    no_excessive_intermediate_flags(mm_helpers::unique_user_virt());
}


#[test_case]
fn t04_huge_page() {
    let _guard = forbid_frame_leaks();

    let mut address_space = BASE_ADDRESS_SPACE.lock();
    let mapping = mapping(&mut address_space);

    let phys2virt = phys2virt(mapping);
    let phys2virt_start = phys2virt.map(Phys::default()).unwrap();

    let result = mapping.path(phys2virt_start).get();
    assert_eq!(result, Err(Unimplemented));

    let result = mapping.path(phys2virt_start).map_intermediate(USER);
    assert_eq!(result, Err(Unimplemented));
}


#[test_case]
fn t05_no_page() {
    let _guard = forbid_frame_leaks();

    let mut address_space = BASE_ADDRESS_SPACE.lock();
    let mapping = mapping(&mut address_space);

    let virt = mm_helpers::unique_user_virt();
    let pte = mapping.translate(virt);

    assert_eq!(pte, Err(NoPage));
}


#[test_case]
fn t06_build_map() {
    let _guard = forbid_frame_leaks();

    let mut address_space = BASE_ADDRESS_SPACE.lock();
    let mapping = mapping(&mut address_space);

    let flag_mask = !(PageTableFlags::ACCESSED | PageTableFlags::DIRTY);

    for ignore_frame_addresses in [false, true] {
        if ignore_frame_addresses {
            debug!("virtual address space");
        } else {
            debug!("virtual to physical mapping");
        }

        for block in mapping
            .iter_mut()
            .map(|path| path.block().unwrap())
            .coalesce(|a, b| a.coalesce(b, ignore_frame_addresses, flag_mask))
        {
            debug!(%block);
        }
    }
}


#[test_case]
fn t07_duplicate_drop() {
    let _guard = forbid_frame_leaks();

    let start_free_frames = FRAME_ALLOCATOR.lock().count();

    let mut address_space = BASE_ADDRESS_SPACE.lock();

    let a = mm_helpers::unique_user_virt();
    let b = mm_helpers::unique_user_virt();

    {
        let original_mapping = mapping(&mut address_space);
        original_mapping.path(a).map_intermediate(USER).unwrap();
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
        let mut address_space_copy = duplicate(address_space).unwrap();
        let mut mapping_copy = mapping(&mut address_space_copy);
        let mut original_mapping = mapping(address_space);

        {
            let start_free_frames = FRAME_ALLOCATOR.lock().count();
            mapping_copy.path(virt).map_intermediate(USER).unwrap();
            let end_free_frames = FRAME_ALLOCATOR.lock().count();
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
fn t08_duplicate_path() {
    let _guard = forbid_frame_leaks();

    let mut address_space_copy = duplicate(&mut BASE_ADDRESS_SPACE.lock()).unwrap();
    switch_to(&address_space_copy);

    path(&mut address_space_copy);
}


#[test_case]
fn t09_duplicate_translate() {
    let _guard = forbid_frame_leaks();

    let mut address_space_copy = duplicate(&mut BASE_ADDRESS_SPACE.lock()).unwrap();
    switch_to(&address_space_copy);

    translate(&mut address_space_copy);
}


#[test_case]
fn t10_drop_subtree() {
    BASE_ADDRESS_SPACE.lock().mapping().unmap_unused_intermediate();

    {
        let start_free_frames = FRAME_ALLOCATOR.lock().count();

        map_intermediate(mm_helpers::unique_user_virt());
        assert!(FRAME_ALLOCATOR.lock().count() < start_free_frames);

        let mut address_space = BASE_ADDRESS_SPACE.lock();
        let mapping = mapping(&mut address_space);

        mapping.unmap_unused_intermediate();
        let mid_free_frames = FRAME_ALLOCATOR.lock().count();

        mapping.unmap_unused_intermediate();
        let end_free_frames = FRAME_ALLOCATOR.lock().count();
        if start_free_frames < end_free_frames {
            panic!("some frames are double freed, check that drop_subtree() clears relevant present flags");
        }
        assert_eq!(start_free_frames, mid_free_frames);
        assert_eq!(start_free_frames, end_free_frames);
    }

    {
        let _guard = forbid_frame_leaks();
        map_intermediate(mm_helpers::unique_user_virt());
    }

    {
        let _guard = forbid_frame_leaks();
        no_excessive_intermediate_flags(mm_helpers::unique_user_virt());
    }
}


#[test_case]
fn t11_no_frame() {
    let mut address_space = BASE_ADDRESS_SPACE.lock();
    let mapping = mapping(&mut address_space);

    let frame_count = FRAME_ALLOCATOR.lock().count();
    let mut real_frame_count = 0;

    while FRAME_ALLOCATOR.lock().allocate().is_ok() {
        real_frame_count += 1;
    }

    debug!(
        frame_count,
        real_frame_count,
        "real free frame count can be less than free frame count reported by boot frame allocator since it records reference and deallocation counts but does not do that actually",
    );
    assert!(real_frame_count <= frame_count);

    let virt = mm_helpers::unique_user_virt();
    let pte = mapping.path(virt).map_intermediate(PageTableFlags::empty());
    assert_eq!(pte, Err(NoFrame));
}


fn check_intermediate_flags(mapping: &mut Mapping, virt: Virt, expected_flags: PageTableFlags) {
    let page_table_root_index_shift =
        PAGE_TABLE_ROOT_LEVEL * PAGE_TABLE_INDEX_BITS + PAGE_OFFSET_BITS;
    let page_table_root_index =
        (virt.into_usize() >> page_table_root_index_shift) & PAGE_TABLE_INDEX_MASK;
    let intermediate_flags = page_table_root(mapping)[page_table_root_index].flags();
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


fn check_path(mapping: &mut Mapping, virt: Virt) {
    let page_table_root = Page::new(Virt::from_ref(page_table_root(mapping))).unwrap();
    let path = &mapping.path(virt);

    assert_eq!(
        Page::containing(nodes(path)[size::from(PAGE_TABLE_ROOT_LEVEL)]),
        page_table_root,
        "PTE at PAGE_TABLE_ROOT_LEVEL in any Path should be the inside the root page table",
    );

    assert!(
        nodes(path).iter().is_partitioned(|&pte| pte == Virt::default()),
        "only lower PTEs can be absent in a Path",
    );

    let (level, pte) = deepest_pte(&path);
    if level > 0 {
        assert!(!pte.present() || pte.flags().contains(PageTableFlags::HUGE_PAGE));
    }

    for (&child, &parent) in nodes(path).iter().zip(nodes(path)[1..].iter()) {
        if child != Virt::default() {
            let child_page = Page::containing(child);
            let phys2virt_start_page =
                Page::containing(phys2virt(mapping).map(Phys::default()).unwrap());
            let child_frame_index = (child_page - phys2virt_start_page).unwrap();
            let parent_points_to =
                unsafe { parent.try_into_ref::<PageTableEntry>().unwrap().frame().unwrap() };
            assert_eq!(child_frame_index, parent_points_to.index());
        }
    }
}
