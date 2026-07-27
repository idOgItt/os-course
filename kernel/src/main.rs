#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

#![feature(custom_test_frameworks)]
#![feature(maybe_uninit_slice)]
#![no_std]
#![no_main]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


extern crate alloc;


use core::{fmt::Write, panic::PanicInfo};

use bootloader::{entry_point, BootInfo};

use ku::{
    self,
    backtrace::Backtrace,
    sync,
    time::{pit8254::Pit, rtc::Rtc},
};

#[cfg(not(feature = "conservative-backtraces"))]
use macros::with_sentinel_frame;

use text::{println, Attribute};

use kernel::{
    self,
    log::debug,
    time,
    trap::{Trap, TRAP_STATS},
    ExitCode,
    Subsystems,
};


entry_point!(kernel_main);


#[cfg_attr(not(feature = "conservative-backtraces"), with_sentinel_frame)]
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    kernel::init_subsystems(boot_info, Subsystems::empty());

    #[cfg(test)]
    test_main();

    while TRAP_STATS[Trap::Rtc].count() < 10 {
        if let Some(frequency) = Pit::tsc_per_second() {
            debug!(%frequency, "CPU frequency measured by PIT");
        }

        if let Some(frequency) = Rtc::tsc_per_second() {
            debug!(%frequency, "CPU frequency measured by RTC");
        }

        let time_precision = time::timer().lap();
        debug!(%time_precision, time_precision_in_tsc = ?time_precision);

        for (number, stats) in TRAP_STATS.iter().enumerate() {
            let count = stats.count();
            if count != 0 {
                let mnemonic = stats.mnemonic();
                debug!(number, %mnemonic, count, "interrupt stats");
            }
        }

        x86_64::instructions::hlt();
    }

    kernel::exit_qemu(ExitCode::SUCCESS);
}


#[cold]
#[inline(never)]
#[panic_handler]
fn panic(panic_info: &PanicInfo) -> ! {
    sync::start_panicing();

    if cfg!(test) {
        kernel::fail_test(panic_info)
    } else {
        text::TEXT.lock().set_attribute(Attribute::PANIC);

        println!("{panic_info}");

        if let Ok(backtrace) = Backtrace::current() {
            println!("{backtrace:?}");
        }

        unsafe { ku::halt() }
    }
}
