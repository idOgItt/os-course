#![deny(warnings)]
#![feature(custom_test_frameworks)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


use kernel::{
    memory::test_scaffolding::forbid_frame_leaks,
    process::{test_scaffolding::disable_interrupts, Scheduler, Table},
    trap::{Trap, TRAP_STATS},
    Subsystems,
};


mod gen_main;
mod process_helpers;


gen_main!(Subsystems::MEMORY | Subsystems::SMP | Subsystems::PROCESS);


const TRAP_HANDLER_ELF: &[u8] = page_aligned!("../../target/kernel/user/trap_handler");


#[test_case]
fn trap_handler() {
    let _guard = forbid_frame_leaks();

    let pid = process_helpers::allocate(TRAP_HANDLER_ELF).pid();

    Scheduler::enqueue(pid);

    while Scheduler::run_one() {
        if let Ok(mut process) = Table::get(pid) {
            disable_interrupts(&mut process);
        }
    }

    let expected_page_faults = 1 + MAX_RECURSION_LEVEL + 2;

    assert_eq!(
        TRAP_STATS[Trap::PageFault].count(),
        expected_page_faults,
        "trap_handler should page fault once on a non-recursive page fault and MAX_RECURSION_LEVEL + 2 on a recursive one",
    );
}


const MAX_RECURSION_LEVEL: usize = 8;
