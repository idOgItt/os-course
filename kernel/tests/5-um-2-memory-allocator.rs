#![feature(custom_test_frameworks)]
#![feature(iterator_try_reduce)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


use chrono::Duration;

use ku::{
    error::Error::{self, InvalidArgument, NoPage, Overflow, PermissionDenied, WrongAlignment},
    memory::{
        mmu::{KERNEL_READ, KERNEL_RW, USER_READ, USER_RW},
        Block,
        Page,
    },
    process::{Pid, SyscallResult},
    sync::spinlock::Spinlock,
};

use kernel::{
    log::debug,
    memory::test_scaffolding::{forbid_frame_leaks, switch_to},
    process::{
        test_scaffolding::{copy_mapping, map, set_pid, unmap},
        Process,
        Scheduler,
    },
    time::{self, TscDuration},
    trap::{Trap, TRAP_STATS},
    Subsystems,
};


mod gen_main;
mod process_helpers;


gen_main!(Subsystems::MEMORY | Subsystems::SYSCALL | Subsystems::SMP | Subsystems::PROCESS);


const MEMORY_ALLOCATOR_ELF: &[u8] = page_aligned!("../../target/kernel/user/memory_allocator");


#[test_case]
fn map_syscall_group() {
    let _guard = forbid_frame_leaks();

    let process = Spinlock::new(process_helpers::make(MEMORY_ALLOCATOR_ELF));
    set_pid(&mut process.lock(), Pid::new(0));
    let pid = process.lock().pid().into_usize();
    switch_to(process.lock().address_space());

    let block = Block::from_slice("some kernel memory".as_bytes()).enclosing();
    map_over_kernel(&process, block, &[PermissionDenied]);

    let block = process.lock().address_space().allocate(2 * Page::SIZE).unwrap();
    let mut head = block;
    let tail = head.tail(1).unwrap();
    unsafe {
        process.lock().address_space().map_block(tail, KERNEL_RW).unwrap();
    }
    map_over_kernel(&process, block, &[NoPage, PermissionDenied]);
    unsafe {
        process.lock().address_space().unmap_block(tail).unwrap();
    }

    let block = process.lock().address_space().allocate(block.size()).unwrap();
    let address = block.start_address().into_usize();
    let size = block.size();
    let new_block = process.lock().address_space().allocate(block.size()).unwrap();
    let new_address = new_block.start_address().into_usize();

    for flags in [KERNEL_READ, KERNEL_RW] {
        let flags = flags.bits();
        assert_eq!(
            map(process.lock(), pid, address, size, flags),
            Err(PermissionDenied),
        );
    }

    for flags in [USER_READ, USER_RW] {
        let flags = flags.bits();
        assert_eq!(
            map(process.lock(), pid, address, size, flags),
            Ok(SyscallResult(address)),
        );

        let allocated_memory = unsafe { block.try_into_slice::<usize>().unwrap() };
        assert!(
            allocated_memory.iter().all(|&x| x == 0),
            "do not leak information into the user space in allocated frames",
        );

        for flags in [KERNEL_READ, KERNEL_RW] {
            let flags = flags.bits();
            assert_eq!(
                copy_mapping(process.lock(), pid, address, new_address, size, flags),
                Err(PermissionDenied),
            );
        }

        assert!(copy_mapping(process.lock(), pid, address, new_address, size, flags).is_ok());
    }

    let flags = USER_RW.bits();
    assert!(unmap(process.lock(), pid, address, size).is_ok());
    assert!(unmap(process.lock(), pid, new_address, size).is_ok());
    assert_eq!(
        copy_mapping(process.lock(), pid, address, new_address, size, flags),
        Err(NoPage),
    );
    assert!(unmap(process.lock(), pid, address, size).is_err());

    assert!(map(process.lock(), pid, 0, size, flags).is_ok());
    assert_eq!(
        map(process.lock(), pid, 1, size, flags),
        Err(WrongAlignment),
    );
    for address in [0, address] {
        assert_eq!(
            map(process.lock(), pid, address, 0, flags),
            Err(InvalidArgument),
        );
        for size in [1, size + 1] {
            assert_eq!(
                map(process.lock(), pid, address, size, flags),
                Err(WrongAlignment),
            );
        }
    }

    for (address, size) in [
        (0x1_0000, 0xFFFF_FFFF_0000_0000),
        (0xFFFF_FFFF_FFFF_0000, 0x10_0000),
    ] {
        let result = map(process.lock(), pid, address, size, flags);
        assert!(
            result == Err(InvalidArgument) || result == Err(Overflow),
            "expected Err(InvalidArgument) or Err(Overflow), got {:?}",
            result,
        );
    }
}


fn map_over_kernel(process: &Spinlock<Process>, block: Block<Page>, expected_errors: &[Error]) {
    let pid = process.lock().pid().into_usize();

    let address = block.start_address().into_usize();
    let size = block.size();
    let new_block = process.lock().address_space().allocate(block.size()).unwrap();
    let new_address = new_block.start_address().into_usize();

    let error = unmap(process.lock(), pid, address, size).unwrap_err();
    assert!(
        expected_errors.iter().any(|x| *x == error),
        "expected one of {:?} but got {:?}",
        expected_errors,
        error,
    );

    for flags in [KERNEL_READ, KERNEL_RW, USER_READ, USER_RW] {
        let flags = flags.bits();

        assert_eq!(
            map(process.lock(), pid, address, size, flags),
            Err(PermissionDenied),
        );
        assert_eq!(
            copy_mapping(process.lock(), pid, address, new_address, size, flags),
            Err(PermissionDenied),
        );
    }
}


#[test_case]
fn copy_mapping_of_intersecting_blocks() {
    let _guard = forbid_frame_leaks();

    let process = Spinlock::new(process_helpers::make(MEMORY_ALLOCATOR_ELF));
    set_pid(&mut process.lock(), Pid::new(0));
    let pid = process.lock().pid().into_usize();
    switch_to(process.lock().address_space());

    let block = process.lock().address_space().allocate(5 * Page::SIZE).unwrap();
    let old = block.slice(1..block.count() - 1).unwrap();
    let old_address = old.start_address().into_usize();
    let size = old.size();
    let flags = USER_READ.bits();

    let mut transactional = true;
    let mut intersection_is_supported = true;

    let try_unmap_page = |page: Page| -> bool {
        match unmap(process.lock(), pid, page.address().into_usize(), Page::SIZE) {
            Ok(_) => true,
            Err(error) => {
                assert_eq!(error, NoPage);
                false
            },
        }
    };

    for hole in [None, old.into_iter().skip(1).next()] {
        let mut results = [
            Ok(SyscallResult::default()),
            Ok(SyscallResult::default()),
            Ok(SyscallResult::default()),
        ];

        for (new_offset, result) in (0..=2).zip(results.iter_mut()) {
            let new = block.slice(new_offset..block.count() - (2 - new_offset)).unwrap();
            let new_address = new.start_address().into_usize();

            for page in old {
                assert!(!try_unmap_page(page));
            }

            assert_eq!(
                map(process.lock(), pid, old_address, size, flags,),
                Ok(SyscallResult(old_address)),
            );

            if let Some(hole) = hole {
                assert!(try_unmap_page(hole));
            }

            for page in new {
                if !old.contains(page) {
                    assert!(!try_unmap_page(page));
                }
            }

            *result = copy_mapping(process.lock(), pid, old_address, new_address, size, flags);

            debug!(%old, %new, ?hole, ?result, "copy mapping of intersecting blocks");

            if hole.is_some() {
                assert_eq!(*result, Err(NoPage));

                for page in new {
                    if !old.contains(page) && try_unmap_page(page) {
                        debug!(%page, "after a copy_mapping() syscall failure a page in the new block is mapped");
                        transactional = false;
                    }
                }
            } else {
                for page in new {
                    if !old.contains(page) {
                        assert!(try_unmap_page(page));
                    }
                }

                if result.is_err() {
                    intersection_is_supported = false;
                    assert_eq!(*result, Err(InvalidArgument));
                }
            }

            for page in old {
                assert_eq!(try_unmap_page(page), hole != Some(page));
            }
        }

        assert!(
            results.iter().try_reduce(|x, y| if x == y { Some(x) } else { None }).is_some(),
            "copy_mapping() result for intersecting blocks is inconsistent: {:?}",
            results,
        );
    }

    debug!(
        intersection_is_supported,
        transactional,
        "{}",
        if intersection_is_supported && transactional {
            "congratulations, copy_mapping() is transactional and supports intersecting blocks"
        } else {
            "copy_mapping()"
        }
    );
}


#[test_case]
fn copy_mapping_of_enormous_blocks() {
    let _guard = forbid_frame_leaks();

    let process = Spinlock::new(process_helpers::make(MEMORY_ALLOCATOR_ELF));
    set_pid(&mut process.lock(), Pid::new(0));
    let pid = process.lock().pid().into_usize();
    switch_to(process.lock().address_space());

    let mut old = process.lock().address_space().allocate(Page::SIZE << 20).unwrap();
    let new = old.tail(old.count() / 2).unwrap();

    let old_address = old.start_address().into_usize();
    let new_address = new.start_address().into_usize();
    let size = old.size();
    let flags = USER_READ.bits();

    let iterations = 1_000;
    let timeout = TscDuration::try_from(Duration::seconds(2)).unwrap();
    let timer = time::timer();

    for iteration in 1..=iterations {
        assert_eq!(
            copy_mapping(process.lock(), pid, old_address, new_address, size, flags),
            Err(NoPage),
        );

        let elapsed = timer.elapsed();

        assert!(
            elapsed < timeout,
            "only {} iterations of copy_mapping() in {}",
            iteration,
            elapsed,
        );
    }

    let elapsed = timer.elapsed();
    debug!(iterations, %elapsed, "iterations of copy_mapping()");
}


#[test_case]
fn user_space_memory_allocator() {
    let _guard = forbid_frame_leaks();

    Scheduler::enqueue(process_helpers::allocate(MEMORY_ALLOCATOR_ELF).pid());

    while Scheduler::run_one() {}

    assert!(
        TRAP_STATS[Trap::PageFault].count() == 0,
        "the user mode code has detected an error in the memory allocator implementation",
    );
}
