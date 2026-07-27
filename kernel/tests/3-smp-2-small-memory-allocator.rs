#![feature(allocator_api)]
#![feature(custom_test_frameworks)]
#![feature(ptr_as_uninit)]
#![feature(slice_ptr_get)]
#![feature(strict_provenance)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


extern crate alloc;

use alloc::{
    alloc::{Allocator, Global, Layout},
    boxed::Box,
    collections::BTreeMap,
    vec::Vec,
};
use core::mem;

use ku::memory::{size::MiB, Page, Virt};

use kernel::{allocator, log::debug, Subsystems};


mod gen_main;

gen_main!(Subsystems::MEMORY);


#[test_case]
fn basic() {
    memory_allocator_basic();
}


#[test_case]
fn alignment() {
    memory_allocator_alignment();
}


#[test_case]
fn grow_and_shrink() {
    memory_allocator_grow_and_shrink();
}


#[test_case]
fn stress() {
    let values = 100_000;
    let pages_for_values = memory_allocator_stress(values);
    debug!(values, pages_for_values);
    assert!(pages_for_values < 2 * values / mem::size_of::<usize>());
}


macro_rules! my_assert {
    ($cond:expr$(,)?) => {{
        assert!($cond);
    }};
}


include!("../../tests/memory_allocator.rs");
