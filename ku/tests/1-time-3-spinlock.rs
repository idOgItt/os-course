#![deny(warnings)]


#[cfg(not(miri))]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{sync::Arc, thread, time::Duration};

use derive_more::{Add, Sum};
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
#[timeout(Duration::from_secs(10))]
fn exclusive_access() {
    log::init();

    let mut spinlock = Spinlock::new(0);

    *spinlock.get_mut() += 1;
    debug!(?spinlock, "unlocked");

    assert_eq!(*spinlock.lock(), 1);
}


#[cfg(not(miri))]
#[rstest]
#[timeout(Duration::from_secs(10))]
fn deadlock() {
    log::init();

    let a = Arc::new(Spinlock::new(0));
    let b = Arc::new(Spinlock::new(1));
    let barrier = Arc::new(AtomicUsize::new(0));

    let (lock_a, lock_b) = (a.lock(), b.lock());
    debug!(?a, ?b, "attempting create a deadlock on two spinlocks");

    fn acquire_two_spinlocks(x: &Spinlock<i32>, y: &Spinlock<i32>, barrier: &AtomicUsize) {
        let lock_x = x.lock();
        debug!(?lock_x, "acquired first lock");

        barrier.fetch_add(1, Ordering::Relaxed);
        loop {
            let arrived = barrier.load(Ordering::Relaxed);
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

    let ab = {
        let a = a.clone();
        let b = b.clone();
        let barrier = barrier.clone();

        thread::spawn(move || acquire_two_spinlocks(&a, &b, &barrier))
    };

    let ba = {
        let a = a.clone();
        let b = b.clone();

        thread::spawn(move || acquire_two_spinlocks(&b, &a, &barrier))
    };

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

    const ITERATION_COUNT: usize = if cfg!(miri) { 500 } else { 100_000 };
    const THREAD_COUNT: usize = if cfg!(miri) { 5 } else { 50 };

    let spinlock = Arc::new(Spinlock::new((0, 0)));

    fn check_spinlock(spinlock: &Spinlock<(usize, usize)>) -> Stats {
        let mut report = true;
        let mut stats = Stats::default();

        for iteration in 0..ITERATION_COUNT {
            let mut lock = spinlock.lock();
            let data = *lock;

            if 2 * data.0 == data.1 {
                if data.0 > 0 {
                    stats.consistent += 1;
                }
            } else {
                if report {
                    error!(?data, iteration, "inconsistent data");
                    report = false;
                }
                stats.inconsistent += 1;
            }

            let i = data.0;
            lock.0 = i + 1;

            thread::yield_now();

            lock.1 = 2 * i + 2;
        }

        stats.completed = 1;

        stats
    }

    let threads: Vec<_> = (0..THREAD_COUNT)
        .map(|thread| {
            let spinlock = spinlock.clone();

            thread::Builder::new()
                .name(format!("thread #{}", thread))
                .spawn(move || check_spinlock(&spinlock))
                .unwrap()
        })
        .collect();

    let stats = threads.into_iter().map(|thread| thread.join().unwrap_or_default()).sum();
    let Stats {
        completed,
        consistent,
        inconsistent,
    } = stats;

    let false_positive_count = THREAD_COUNT - completed;
    debug!(false_positive_count, "false positive deadlock detections");

    debug!(consistent, inconsistent);
    assert!(
        consistent > ITERATION_COUNT / 10,
        "detected only {} consistent data reads",
        consistent,
    );
    assert_eq!(
        inconsistent, 0,
        "detected {} inconsistent data reads",
        inconsistent,
    );

    debug!(?spinlock);
}


#[derive(Add, Clone, Copy, Default, Sum)]
struct Stats {
    completed: usize,
    consistent: usize,
    inconsistent: usize,
}
