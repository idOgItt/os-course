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

use ku::process::Pid;

use kernel::{
    log::debug,
    memory::{test_scaffolding::forbid_frame_leaks, FRAME_ALLOCATOR},
    process::{test_scaffolding::set_pid_callback, Process, Scheduler},
    trap::{Trap, TRAP_STATS},
    Subsystems,
};


mod gen_main;
mod process_helpers;


gen_main!(Subsystems::MEMORY | Subsystems::SMP | Subsystems::PROCESS);


const COW_FORK_ELF: &[u8] = page_aligned!("../../target/kernel/user/cow_fork");


#[test_case]
fn cow_fork() {
    static mut START_FREE_FRAMES: usize = 0;
    static mut ONE_PROCESS_FRAMES: usize = 0;
    static mut PARENTS: Vec<(Pid, Pid)> = Vec::new();

    unsafe {
        PARENTS = Vec::with_capacity(16);
    }

    set_pid_callback(record_parent);
    fn record_parent(process: &Process) {
        if let Some(parent) = process.parent() {
            unsafe {
                PARENTS.push((parent, process.pid()));
                let used_frames = START_FREE_FRAMES - FRAME_ALLOCATOR.lock().count();
                assert!(
                    used_frames < 70 * (1 + PARENTS.len()) * ONE_PROCESS_FRAMES / 100,
                    "less than 30% of the physical frames are shared: {} per process, {} total, {} processes",
                    ONE_PROCESS_FRAMES,
                    used_frames,
                    1 + PARENTS.len(),
                );
            }
        }
    }

    {
        let _guard = forbid_frame_leaks();

        unsafe {
            START_FREE_FRAMES = FRAME_ALLOCATOR.lock().count();
        }
        let pid = process_helpers::allocate(COW_FORK_ELF).pid();
        unsafe {
            ONE_PROCESS_FRAMES = START_FREE_FRAMES - FRAME_ALLOCATOR.lock().count();
        }

        Scheduler::enqueue(pid);

        while Scheduler::run_one() {}
    }

    assert!(
        TRAP_STATS[Trap::PageFault].count() > 100,
        "cow_fork should page fault a lot",
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
