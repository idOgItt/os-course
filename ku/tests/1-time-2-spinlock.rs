#![deny(warnings)]


use std::{
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::Duration,
};

use rstest::rstest;

use ku::{
    log::{debug, error},
    sync::Spinlock,
};


mod log;


#[rstest]
#[timeout(Duration::from_secs(1))]
fn lock_unlock() {
    log::init();

    let spinlock = Spinlock::new(0);

    let mut lock = spinlock.lock();
    *lock += 1;
    debug!(?spinlock, "locked");

    assert!(spinlock.try_lock().is_none());

    drop(lock);
    debug!(?spinlock, "unlocked");

    let lock = spinlock.lock();
    assert_eq!(*lock, 1);
    drop(lock);
}


#[rstest]
#[timeout(Duration::from_secs(1))]
fn try_lock() {
    log::init();

    let spinlock = Spinlock::new(0);

    let lock = spinlock.lock();
    debug!(?spinlock, "locked");

    assert!(spinlock.try_lock().is_none());

    drop(lock);
    debug!(?spinlock, "unlocked");

    assert!(spinlock.try_lock().is_some());
}


#[rstest]
#[timeout(Duration::from_secs(1))]
fn exclusive_access() {
    log::init();

    let mut spinlock = Spinlock::new(0);

    *spinlock.get_mut() += 1;
    debug!(?spinlock, "unlocked");

    assert_eq!(*spinlock.lock(), 1);
}


#[rstest]
#[timeout(Duration::from_secs(10))]
fn deadlock() {
    log::init();

    static A: Spinlock<i32> = Spinlock::new(0);
    static B: Spinlock<i32> = Spinlock::new(1);
    static BARRIER: AtomicUsize = AtomicUsize::new(0);

    let (lock_a, lock_b) = (A.lock(), B.lock());
    debug!(?A, ?B, "attempting create a deadlock on two spinlocks");

    fn acquire_two_spinlocks(x: &'static Spinlock<i32>, y: &'static Spinlock<i32>) {
        let lock_x = x.lock();
        debug!(?lock_x, "acquired first lock");

        BARRIER.fetch_add(1, Ordering::Relaxed);
        loop {
            let arrived = BARRIER.load(Ordering::Relaxed);
            debug!(arrived, "waiting on a barrier");
            if arrived >= 2 {
                break;
            }
            thread::yield_now();
        }

        let lock_y = y.lock();

        drop(lock_y);
        drop(lock_x);
    }

    let ab = thread::spawn(move || {
        acquire_two_spinlocks(&A, &B);
    });

    let ba = thread::spawn(move || {
        acquire_two_spinlocks(&B, &A);
    });

    drop(lock_a);
    drop(lock_b);

    let ab_result = ab.join();
    let ba_result = ba.join();
    debug!(?ab_result, ?ba_result);
    assert!(
        ab_result.is_err() || ba_result.is_err(),
        "at least one of the results should contain an error: {:?}, {:?}",
        ab_result,
        ba_result,
    );
}


#[rstest]
#[timeout(Duration::from_secs(60))]
fn concurrent() {
    log::init();

    const ITERATION_COUNT: usize = 100_000;
    const THREAD_COUNT: usize = 100;

    static CONSISTENT: AtomicUsize = AtomicUsize::new(0);
    static INCONSISTENT: AtomicUsize = AtomicUsize::new(0);
    static SPINLOCK: Spinlock<(usize, usize)> = Spinlock::new((0, 0));

    fn check_spinlock() {
        let mut consistent = 0;
        let mut inconsistent = 0;
        let mut report = true;

        for iteration in 0..ITERATION_COUNT {
            let mut lock = SPINLOCK.lock();
            let data = *lock;

            if 2 * data.0 == data.1 {
                if data.0 > 0 {
                    consistent += 1;
                }
            } else {
                if report {
                    error!(?data, iteration, "inconsistent data");
                    report = false;
                }
                inconsistent += 1;
            }

            let i = data.0;
            lock.0 = i + 1;

            thread::yield_now();

            lock.1 = 2 * i + 2;
        }

        CONSISTENT.fetch_add(consistent, Ordering::Relaxed);
        INCONSISTENT.fetch_add(inconsistent, Ordering::Relaxed);
    }

    let mut threads = Vec::new();

    for thread in 0..THREAD_COUNT {
        threads.push(
            thread::Builder::new()
                .name(format!("thread #{}", thread))
                .spawn(move || {
                    check_spinlock();
                })
                .unwrap(),
        );
    }

    let mut false_positive_count = 0;
    while let Some(thread) = threads.pop() {
        if thread.join().is_err() {
            false_positive_count += 1;
        }
    }
    debug!(false_positive_count, "false positive deadlock detections");

    let consistent_count = CONSISTENT.load(Ordering::Relaxed);
    let inconsistent_count = INCONSISTENT.load(Ordering::Relaxed);
    debug!(consistent_count, inconsistent_count);
    assert!(
        consistent_count > ITERATION_COUNT / 10,
        "detected only {} consistent data reads",
        consistent_count,
    );
    assert_eq!(
        inconsistent_count, 0,
        "detected {} inconsistent data reads",
        inconsistent_count,
    );

    debug!(spinlock = ?SPINLOCK);
}
