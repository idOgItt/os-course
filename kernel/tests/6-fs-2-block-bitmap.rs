#![feature(asm_const)]
#![feature(custom_test_frameworks)]
#![feature(naked_functions)]
#![feature(slice_group_by)]
#![feature(sort_internals)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


extern crate alloc;

use alloc::vec::Vec;
use core::slice;

use ku::error::Error::NoDisk;

use kernel::{
    fs::{BlockCache, Kind},
    log::debug,
    Subsystems,
};


mod fs_helpers;
mod gen_main;


gen_main!(Subsystems::MEMORY);


#[test_case]
fn allocation() {
    let (mut block_bitmap, _, _) = fs_helpers::simple_fs(Kind::File);

    let free_block_count = (0..fs_helpers::BLOCK_COUNT)
        .map(|i| if block_bitmap.is_free(i) { 1 } else { 0 })
        .sum();
    assert_eq!(
        free_block_count,
        fs_helpers::BLOCK_COUNT - fs_helpers::simple_fs_superblock().blocks().start,
    );

    let mut allocated: Vec<_> =
        (0..free_block_count).map(|_| block_bitmap.allocate().unwrap()).collect();
    debug!(
        allocated_head = ?allocated[..10],
        allocated_tail = ?allocated[allocated.len() - 10..],
    );

    assert!(block_bitmap.allocate() == Err(NoDisk));
    block_bitmap.set_free(fs_helpers::simple_fs_superblock().blocks().start + 5);
    block_bitmap.allocate().unwrap();
    for _ in (1..=fs_helpers::BLOCK_COUNT).rev() {
        block_bitmap.set_free(fs_helpers::BLOCK_COUNT - 33);
        block_bitmap.allocate().unwrap();
    }

    slice::heapsort(&mut allocated, |a, b| a < b);
    for (i, j) in allocated.iter().zip(allocated.iter().skip(1)) {
        assert_ne!(*i, *j, "allocated the same block twice");
    }

    let mut free_block_count = 0;
    for i in allocated.iter().step_by(2) {
        if i % 10_000 == 0 {
            debug!(block = i, free_block_count);
        }
        assert!(!block_bitmap.is_free(*i));
        block_bitmap.set_free(*i);
        free_block_count += 1;
    }

    let mut reallocated: Vec<_> =
        (0..free_block_count).map(|_| block_bitmap.allocate().unwrap()).collect();
    debug!(
        reallocated_head = ?reallocated[..10],
        reallocated_tail = ?reallocated[reallocated.len() - 10..],
    );
    assert!(block_bitmap.allocate() == Err(NoDisk));

    slice::heapsort(&mut reallocated, |a, b| a < b);
    for (i, j) in allocated.iter().step_by(2).zip(reallocated) {
        assert_eq!(*i, j);
    }

    debug!(block_cache_stats = ?BlockCache::stats());
}
