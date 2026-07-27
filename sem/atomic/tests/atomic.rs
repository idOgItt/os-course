#![deny(warnings)]


use std::{
    hint,
    ptr,
    sync::atomic::{AtomicBool, Ordering},
    thread,
};

use ntest_timeout::timeout;

use atomic::Atomic;


#[test]
#[timeout(60_000)]
fn add_sub() {
    static ATOMIC: Atomic = Atomic::new(0);

    run_threads(move || summer1(&ATOMIC));
    assert_eq!(ATOMIC.load(), ITERATIONS * THREADS);

    run_threads(move || subber1(&ATOMIC));
    assert_eq!(ATOMIC.load(), 0);

    run_threads(move || summer2(&ATOMIC));
    assert_eq!(ATOMIC.load(), 2 * ITERATIONS * THREADS);

    run_threads(move || subber2(&ATOMIC));
    assert_eq!(ATOMIC.load(), 0);
}


#[test]
#[timeout(60_000)]
fn xchg() {
    static FLAG: Atomic = Atomic::new(1);
    static mut COUNTER: usize = 0;

    run_threads(move || exchanger(&FLAG, unsafe { &mut *ptr::addr_of_mut!(COUNTER) } as *mut usize));
}


#[test]
#[timeout(60_000)]
fn cmpxchg() {
    static ATOMIC: Atomic = Atomic::new(1);

    for _ in 0..CMPXCHG_ITERATIONS {
        ATOMIC.store(1);

        run_threads(move || compare_exchanger(&ATOMIC));

        assert_eq!(ATOMIC.load(), 2 * THREADS + (THREADS + 1) % 2);
    }
}


fn run_threads(f: fn()) {
    static WAIT_AT_BARRIER: AtomicBool = AtomicBool::new(true);

    let mut threads = Vec::new();
    for _ in 0..THREADS {
        threads.push(thread::spawn(move || {
            while WAIT_AT_BARRIER.load(Ordering::Relaxed) {
                hint::spin_loop();
            }

            f();
        }));
    }

    WAIT_AT_BARRIER.store(false, Ordering::Relaxed);

    while let Some(thread) = threads.pop() {
        assert!(thread.join().is_ok());
    }
}


fn summer1(atomic: &Atomic) {
    for _ in 0..ITERATIONS {
        atomic.add(1);
    }
}


fn summer2(atomic: &Atomic) {
    for _ in 0..ITERATIONS {
        atomic.add(2);
    }
}


fn subber1(atomic: &Atomic) {
    for _ in 0..ITERATIONS {
        atomic.sub(1);
    }
}


fn subber2(atomic: &Atomic) {
    for _ in 0..ITERATIONS {
        atomic.sub(2);
    }
}


fn exchanger(flag: &Atomic, counter: *mut usize) {
    while flag.exchange(0) != 1 {
        hint::spin_loop();
    }
    unsafe {
        counter.write_volatile(counter.read_volatile() + 1);
    }
    flag.store(1);
}

fn compare_exchanger(atomic: &Atomic) {
    let mut old = atomic.load();
    loop {
        let new = if old & 1 != 0 { old + 1 } else { old + 3 };
        if atomic.compare_exchange(&mut old, new) {
            return;
        }
        hint::spin_loop();
    }
}


const CMPXCHG_ITERATIONS: usize = 20;
const ITERATIONS: usize = 1 << 14;
const THREADS: usize = 1000;
