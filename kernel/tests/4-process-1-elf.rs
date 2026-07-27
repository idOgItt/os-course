#![feature(custom_test_frameworks)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


use kernel::{log::info, memory::test_scaffolding::forbid_frame_leaks, process, Subsystems};


mod gen_main;
mod process_helpers;


gen_main!(Subsystems::MEMORY | Subsystems::SMP);


const LOOP_ELF: &[u8] = page_aligned!("../../target/kernel/user/loop");


#[test_case]
fn create_process() {
    let _guard = forbid_frame_leaks();

    process_helpers::make(LOOP_ELF);
}


#[test_case]
fn create_process_failure() {
    let _guard = forbid_frame_leaks();

    let bad_elf_file: &[u8] = &[];
    let error = process::create(bad_elf_file).expect_err("created a process from a bad ELF file");

    info!(?error, "expected a process creation failure");
}
