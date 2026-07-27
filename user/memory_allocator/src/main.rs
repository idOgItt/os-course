#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

#![feature(allocator_api)]
#![feature(slice_ptr_get)]
#![feature(strict_provenance)]
#![no_main]
#![no_std]


extern crate alloc;

use alloc::{
    alloc::{Allocator, Global, Layout},
    boxed::Box,
    collections::BTreeMap,
    vec::Vec,
};
use core::{mem, panic::PanicInfo, ptr::NonNull};

use ku::{
    log::{debug, error, info},
    memory::{size::MiB, Page, Virt},
};

use lib::{allocator, entry};


entry!(main);


fn main() {
    lib::set_panic_handler(panic_handler);

    info!(test_case = "basic");
    memory_allocator_basic();

    info!(test_case = "alignment");
    memory_allocator_alignment();

    info!(test_case = "grow_and_shrink");
    memory_allocator_grow_and_shrink();

    info!(test_case = "stress");
    memory_allocator_stress(10_000);
}


fn generate_page_fault() -> ! {
    unsafe {
        NonNull::<u8>::dangling().as_ptr().read_volatile();
    }

    unreachable!();
}


fn panic_handler(_: &PanicInfo) {
    generate_page_fault();
}


macro_rules! my_assert {
    ($condition:expr$(,)?) => {{
        if !$condition {
            error!(condition = stringify!($condition), "assert failed");
            generate_page_fault();
        }
    }};
}


include!("../../../tests/memory_allocator.rs");
