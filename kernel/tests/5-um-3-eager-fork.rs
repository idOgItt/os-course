#![deny(warnings)]
#![feature(custom_test_frameworks)]
#![feature(naked_functions)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


extern crate alloc;

use alloc::{format, string::String, vec::Vec};
use core::ptr;

use introsort;

use ku::{
    error::Error::{InvalidArgument, PermissionDenied},
    process::{Pid, State},
};

use kernel::{
    log::debug,
    memory::test_scaffolding::{forbid_frame_leaks, switch_to},
    process::{
        test_scaffolding::{exofork, set_pid_callback, set_state},
        Process,
        Scheduler,
        Table,
    },
    trap::{Trap, TRAP_STATS},
    Subsystems,
};


mod gen_main;
mod process_helpers;


gen_main!(Subsystems::MEMORY | Subsystems::SMP | Subsystems::PROCESS);


const EAGER_FORK_ELF: &[u8] = page_aligned!("../../target/kernel/user/eager_fork");
const EXIT_ELF: &[u8] = page_aligned!("../../target/kernel/user/exit");


#[test_case]
fn t1_exofork_syscall() {
    let _guard = forbid_frame_leaks();

    let mut parent = process_helpers::allocate(EXIT_ELF);
    let parent_pid = parent.pid();
    let unrelated_process_pid = process_helpers::allocate(EXIT_ELF).pid();

    switch_to(parent.address_space());

    let child_pid = Pid::from_usize(exofork(parent).expect("exofork() failed"))
        .expect("wrong child pid from exofork()");

    debug!(%child_pid);

    let child =
        Table::get(child_pid).expect("failed to find the child process in the process table");

    debug!(%child);

    drop(child);

    assert_eq!(
        set_state(
            Table::get(parent_pid).unwrap(),
            unrelated_process_pid.into_usize(),
            State::Runnable.into(),
        ),
        Err(PermissionDenied),
    );

    assert_eq!(
        set_state(Table::get(parent_pid).unwrap(), child_pid.into_usize(), 42),
        Err(InvalidArgument),
    );

    assert_eq!(
        set_state(
            Table::get(parent_pid).unwrap(),
            child_pid.into_usize(),
            State::Running.into(),
        ),
        Err(InvalidArgument),
    );

    let result = set_state(
        Table::get(parent_pid).unwrap(),
        child_pid.into_usize(),
        State::Runnable.into(),
    );
    assert!(result.is_ok());

    assert_eq!(
        set_state(
            Table::get(parent_pid).unwrap(),
            child_pid.into_usize(),
            State::Runnable.into(),
        ),
        Err(PermissionDenied),
    );

    for pid in [parent_pid, unrelated_process_pid] {
        process_helpers::free(pid);
    }

    let start_page_faults = TRAP_STATS[Trap::PageFault].count();

    while Scheduler::run_one() {}

    assert_eq!(
        TRAP_STATS[Trap::PageFault].count(),
        start_page_faults + 1,
        "the child process should page fault",
    );

    Table::get(child_pid).expect_err("the child process has not run up to its completion");
}


#[test_case]
fn t2_eager_fork() {
    let start_page_fault_count = TRAP_STATS[Trap::PageFault].count();

    static mut PARENTS: Vec<(Pid, Pid)> = Vec::new();

    unsafe {
        PARENTS = Vec::with_capacity(16);
    }

    set_pid_callback(record_parent);
    fn record_parent(process: &Process) {
        if let Some(parent) = process.parent() {
            unsafe {
                PARENTS.push((parent, process.pid()));
            }
        }
    }

    {
        let _guard = forbid_frame_leaks();

        let pid = process_helpers::allocate(EAGER_FORK_ELF).pid();

        Scheduler::enqueue(pid);

        while Scheduler::run_one() {}
    }

    assert!(
        TRAP_STATS[Trap::PageFault].count() == start_page_fault_count,
        "eager_fork should not page fault",
    );

    let parents = unsafe { &mut *ptr::addr_of_mut!(PARENTS) };

    assert_eq!(parents.len(), 12, "wrong total number of child processes");

    introsort::sort(parents);

    for children in parents.chunk_by(|a, b| a.0 == b.0) {
        assert_eq!(
            children.len(),
            3,
            "wrong number of children {} for the process {}",
            children.len(),
            children[0].0,
        );
    }

    let mut graphviz =
        String::from("digraph process_tree { node [ style = filled; fillcolor = \"#CCCCCC\"]; ");
    for (parent, process) in parents {
        debug!(%parent, %process);
        graphviz += &format!("\"{}\" -> \"{}\"; ", parent, process)
    }
    graphviz += "}";
    debug!(%graphviz);

    unsafe {
        PARENTS = Vec::new();
    }
}
