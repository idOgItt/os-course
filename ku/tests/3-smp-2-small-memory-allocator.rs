#![deny(warnings)]
#![feature(allocator_api)]


use std::{
    alloc::{GlobalAlloc, Layout},
    cmp,
    mem,
    sync::Arc,
    thread,
};

use derive_more::{Add, Display, Sum};
use rand::{rngs::SmallRng, Rng, SeedableRng};

use ku::{
    allocator::{DetailedInfo, Dispatcher},
    log::{error, info},
    memory::Virt,
    sync::Spinlock,
};

use mmap_fallback::MmapFallback;


mod log;
mod mmap_fallback;


#[test]
fn single_threaded() {
    log::init();

    let allocator = Dispatcher::new(MmapFallback);

    let stats = stress(&allocator, SINGLE_THREADED_ITERATIONS, SEED);

    let detailed_info: Spinlock<DetailedInfo> = Spinlock::new(DetailedInfo::new());
    allocator.detailed_info(&mut detailed_info.lock());

    validate_info(stats, &detailed_info.lock());
}


#[test]
fn multi_threaded() {
    log::init();

    let allocator = Arc::new(Dispatcher::new(MmapFallback));

    let threads: Vec<_> = (0..THREAD_COUNT)
        .map(|thread| {
            let allocator = allocator.clone();

            thread::spawn(move || stress(&*allocator, MULTI_THREADED_ITERATIONS, SEED + thread))
        })
        .collect();

    let stats = threads
        .into_iter()
        .map(|thread| thread.join().expect("threads should finish successfully"))
        .sum();

    let detailed_info: Spinlock<DetailedInfo> = Spinlock::new(DetailedInfo::new());
    allocator.detailed_info(&mut detailed_info.lock());

    validate_info(stats, &detailed_info.lock());
}


fn validate_info(stats: Stats, detailed_info: &DetailedInfo) {
    info!(total = %detailed_info.total());
    println!("{:#}", detailed_info);

    assert!(detailed_info.is_valid());

    let total = detailed_info.total();
    assert_eq!(total.allocations().positive(), stats.allocations);
    assert!(total.allocated().positive() >= stats.requested);
    assert_eq!(total.requested().positive(), stats.requested);
    assert_eq!(total.allocations().balance(), 0);
    assert_eq!(total.allocated().balance(), 0);
    assert_eq!(total.requested().balance(), 0);
    assert_eq!(total.allocated().balance(), 0);
    assert!(total.pages().positive() > 0);
}


fn stress(allocator: &'_ dyn GlobalAlloc, iterations: usize, seed: u64) -> Stats {
    let mut active = Vec::new();
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut stats = Stats::default();

    for iteration in 1..=iterations {
        let operation = rng.gen_range(0..3);
        let index = if active.len() > 0 {
            rng.gen_range(0..active.len())
        } else {
            0
        };

        if iteration % (iterations / 10) == 0 && index < active.len() {
            info!(iteration, allocation = %active[index]);
        }

        if active.len() == 0 || operation == 0 {
            let allocation = Allocation::generate(allocator, &mut rng);
            stats.allocations += 1;
            stats.requested += allocation.layout.size();
            active.push(allocation);
        } else if active.len() > MAX_ACTIVE_ALLOCATIONS || operation == 1 {
            let last = active.pop().unwrap();
            let to_be_freed = if index < active.len() {
                mem::replace(&mut active[index], last)
            } else {
                last
            };
            drop(to_be_freed);
        } else if operation == 2 {
            active[index].resize(&mut rng);
            stats.allocations += 1;
            stats.requested += active[index].layout.size();
        }
    }

    stats
}


#[derive(Display)]
#[display(
    "{{ size: {}, align: {}, ptr: {}, sum: {} }}",
    layout.size(),
    layout.align(),
    ptr,
    sum
)]
struct Allocation<'a> {
    allocator: &'a dyn GlobalAlloc,
    layout: Layout,
    ptr: Virt,
    sum: usize,
}


impl<'a> Allocation<'a> {
    fn generate(allocator: &'a dyn GlobalAlloc, rng: &mut SmallRng) -> Self {
        let align = Self::generate_align(rng);
        let size = Self::generate_size(rng);
        let layout = Layout::from_size_align(size, align).unwrap();

        let zeroed = rng.gen_ratio(1, 2);

        let ptr = Virt::from_ptr(unsafe {
            if zeroed {
                allocator.alloc_zeroed(layout)
            } else {
                allocator.alloc(layout)
            }
        });
        Self::check_ptr(layout, ptr);

        let data = unsafe { ptr.try_into_mut_slice(layout.size()).unwrap() };
        if zeroed {
            assert_eq!(data.iter().find(|&&x| x != 0), None);
        }
        rng.fill(data);

        Self {
            allocator,
            layout,
            ptr,
            sum: Self::sum(data),
        }
    }


    fn resize(&mut self, rng: &mut SmallRng) {
        self.check_sum();

        let new_size = Self::generate_size(rng);
        let new_layout = Layout::from_size_align(new_size, self.layout.align()).unwrap();
        let common_size = cmp::min(self.layout.size(), new_size);

        let old_data = unsafe { self.ptr.try_into_slice(common_size).unwrap() };
        let old_sum = Self::sum(old_data);

        let new_ptr =
            Virt::from_ptr(unsafe { self.allocator.realloc(self.ptr(), self.layout, new_size) });
        Self::check_ptr(new_layout, new_ptr);

        let new_data = unsafe { new_ptr.try_into_slice(common_size).unwrap() };
        let new_sum = Self::sum(new_data);
        assert_eq!(old_sum, new_sum);

        let data = unsafe { new_ptr.try_into_mut_slice(new_size).unwrap() };
        rng.fill(data);

        self.layout = new_layout;
        self.ptr = new_ptr;
        self.sum = Self::sum(data);
    }


    fn check_ptr(layout: Layout, ptr: Virt) {
        let align_difference = ptr.into_usize() % layout.align();
        if align_difference != 0 {
            error!(?layout, %ptr, align_difference, "wrong alignment of allocated pointer");
        }
        assert_eq!(align_difference, 0);
    }


    fn check_sum(&self) {
        let data = unsafe { self.ptr.try_into_mut_slice(self.layout.size()).unwrap() };
        assert_eq!(Self::sum(data), self.sum);
    }


    fn generate_align(rng: &mut SmallRng) -> usize {
        1 << rng.gen_range(0..=MAX_LB_ALIGN)
    }


    fn generate_size(rng: &mut SmallRng) -> usize {
        SIZE_MULTIPLIER * rng.gen_range(1..=DIFFERENT_SIZES)
    }


    fn ptr(&self) -> *mut u8 {
        self.ptr.try_into_mut_ptr().unwrap()
    }


    fn sum(data: &[u8]) -> usize {
        data.iter().map(|&x| usize::from(x)).sum()
    }
}


impl Drop for Allocation<'_> {
    fn drop(&mut self) {
        self.check_sum();

        unsafe {
            self.allocator.dealloc(self.ptr(), self.layout);
        }
    }
}


#[derive(Add, Clone, Copy, Default, Sum)]
struct Stats {
    allocations: usize,
    requested: usize,
}


const DIFFERENT_SIZES: usize = 1024;
const MAX_ACTIVE_ALLOCATIONS: usize = 1_000_000;
const MAX_LB_ALIGN: u32 = 12;
const MULTI_THREADED_ITERATIONS: usize = 200_000;
const SEED: u64 = 314159265;
const SINGLE_THREADED_ITERATIONS: usize = 2_000_000;
const SIZE_MULTIPLIER: usize = 1;
const THREAD_COUNT: u64 = 4;
