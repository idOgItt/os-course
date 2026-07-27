#![feature(custom_test_frameworks)]
#![feature(naked_functions)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


use core::mem;

use ku::memory::size::MiB;

use kernel::{
    fs::{
        test_scaffolding::{block_cache_init, cache, BLOCK_SIZE},
        BlockCache,
    },
    log::debug,
    Subsystems,
};


mod gen_main;


gen_main!(Subsystems::MEMORY);


#[test_case]
fn read_what_was_written() {
    let block_count = FS_SIZE / BLOCK_SIZE;
    debug!(block_count);

    block_cache_init(FS_DISK, block_count, block_count).unwrap();

    let cache = cache().unwrap();

    let len = 16 << 10;
    let slice = unsafe { cache.try_into_mut_slice::<usize>().unwrap() };

    for (i, actual) in slice[..len].iter_mut().enumerate() {
        let block = i * mem::size_of_val(actual) / BLOCK_SIZE;
        let expected = if block % 2 == 0 { i } else { 0 };
        if *actual != expected || i % 777 == 0 {
            debug!(i, block, actual, expected);
        }
        assert_eq!(*actual, expected);
    }

    debug!(block_cache_stats = ?BlockCache::stats());
}


const FS_DISK: usize = 1;
const FS_SIZE: usize = 32 * MiB;
