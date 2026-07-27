#![deny(warnings)]
#![feature(custom_test_frameworks)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


use kernel::{
    log::info,
    trap::{Trap, TRAP_STATS},
    Subsystems,
};


mod gen_main;

gen_main!(Subsystems::empty());


fn emit_breakpoint_interrupt() {
    info!("emiting breakpoint interrupt (int3)");
    x86_64::instructions::interrupts::int3();
}


#[test_case]
fn interrupts_are_working() {
    emit_breakpoint_interrupt();
}


#[test_case]
fn interrupt_counter() {
    let breakpoint_counter = &TRAP_STATS[Trap::Breakpoint];
    let begin_count = breakpoint_counter.count();
    let count = 3;
    for _ in 0..count {
        emit_breakpoint_interrupt();
    }
    let end_count = breakpoint_counter.count();
    info!(begin_count, count, end_count);
    assert!(begin_count + count == end_count);
}
