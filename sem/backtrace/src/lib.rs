#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use std::arch::asm;
use std::ptr;

pub type Backtrace = Vec<*const ()>;

unsafe extern "C" {
    fn __libc_start_main(
        main: *const (),
        argc: i32,
        argv: *const *const i8,
        init: *const (),
        fini: *const (),
        rtld_fini: *const (),
        stack_end: *const (),
    );
}

#[inline(never)]
fn get_return_address() -> *const () {
    let mut addr: *const () = ptr::null();
    unsafe {
        asm!(
        "mov {}, [rbp + 8]",
        out(reg) addr
        );
    }
    addr
}

#[inline(never)]
pub fn backtrace() -> Backtrace {
    let mut addresses: Backtrace = Vec::new();
    let mut rbp: *const *const () = ptr::null();

    unsafe {
        asm!("mov {}, rbp", out(reg) rbp);

        while !rbp.is_null() {
            let return_address = *rbp.add(1) as *const ();

            if return_address.is_null() {
                break;
            }

            addresses.push(return_address);

            rbp = *rbp as *const *const ();
        }
    }

    addresses
}
