#![deny(warnings)]


use std::{
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    thread,
    time::Duration,
};

use ntest_timeout::timeout;

use rwlock::RwLock;


#[test]
#[timeout(1_000)]
fn simple() {
    let rwlock = RwLock::new(0);

    *rwlock.write() = 3;

    let a = rwlock.read();
    let b = rwlock.read();

    assert_eq!(*a, 3);
    assert_eq!(*b, 3);
}


#[test]
#[timeout(120_000)]
fn stress() {
    static CONSISTENT: AtomicUsize = AtomicUsize::new(0);
    static INCONSISTENT: AtomicUsize = AtomicUsize::new(0);
    static RUN: AtomicBool = AtomicBool::new(true);
    static RW_LOCK: RwLock<(usize, usize)> = RwLock::new((0, 0));
    static CURRENT_READERS: AtomicUsize = AtomicUsize::new(0);
    static MANY_READERS: AtomicBool = AtomicBool::new(false);

    let mut reader_threads = Vec::new();
    let mut writer_threads = Vec::new();

    for thread in 0..READERS {
        reader_threads.push(
            thread::Builder::new()
                .name(format!("reader_thread #{}", thread))
                .spawn(move || {
                    reader(
                        &RW_LOCK,
                        &CONSISTENT,
                        &INCONSISTENT,
                        &CURRENT_READERS,
                        &MANY_READERS,
                    );
                })
                .unwrap(),
        );
    }

    for thread in 0..WRITERS {
        writer_threads.push(
            thread::Builder::new()
                .name(format!("writer_thread #{}", thread))
                .spawn(move || {
                    writer(&RW_LOCK, &RUN);
                })
                .unwrap(),
        );
    }

    while let Some(thread) = reader_threads.pop() {
        assert!(thread.join().is_ok());
    }

    RUN.store(false, Ordering::Release);

    while let Some(thread) = writer_threads.pop() {
        assert!(thread.join().is_ok());
    }

    let consistent_count = CONSISTENT.load(Ordering::Relaxed);
    let inconsistent_count = INCONSISTENT.load(Ordering::Relaxed);
    let many_readers = MANY_READERS.load(Ordering::Relaxed);

    eprintln!(
        "consistent_count = {}, inconsistent_count = {}, many_readers = {}",
        consistent_count, inconsistent_count, many_readers,
    );

    assert!(
        consistent_count >= READERS * MIN_DIFFERENT_READS,
        "detected only {} consistent data reads",
        consistent_count,
    );

    assert_eq!(
        inconsistent_count, 0,
        "detected {} inconsistent data reads",
        inconsistent_count,
    );

    assert!(many_readers, "too few concurrent readers");
}


fn reader(
    rwlock: &RwLock<(usize, usize)>,
    global_consistent: &AtomicUsize,
    global_inconsistent: &AtomicUsize,
    current_readers: &AtomicUsize,
    many_readers: &AtomicBool,
) {
    let mut consistent = 0;
    let mut inconsistent = 0;
    let mut iteration = 0;
    let mut prev = 0;
    let mut report = true;

    while iteration < MIN_DIFFERENT_READS {
        let lock = rwlock.read();

        let data = *lock;

        let readers = current_readers.fetch_add(1, Ordering::Relaxed) + 1;
        if readers >= CURRENT_READERS_THRESHOLD {
            many_readers.store(true, Ordering::Relaxed);
        }

        if 2 * data.0 == data.1 {
            if data.0 > 0 {
                consistent += 1;
            }
        } else {
            if report {
                eprintln!("inconsistent data={:?} iteration={}", data, iteration);
                report = false;
            }
            inconsistent += 1;
        }

        if data.0 != prev {
            prev = data.0;
            iteration += 1;
        }

        thread::sleep(Duration::from_millis(1));

        current_readers.fetch_sub(1, Ordering::Relaxed);
    }

    global_consistent.fetch_add(consistent, Ordering::Relaxed);
    global_inconsistent.fetch_add(inconsistent, Ordering::Relaxed);
}


fn writer(rwlock: &RwLock<(usize, usize)>, run: &AtomicBool) {
    while run.load(Ordering::Acquire) {
        let mut lock = rwlock.write();
        let data = *lock;

        let i = data.0;
        lock.0 = i + 1;

        if i % WRITERS == 0 {
            thread::yield_now();
        }

        lock.1 = 2 * i + 2;

        drop(lock);

        thread::sleep(Duration::from_millis(if cfg!(miri) { 100 } else { 10 }));
    }
}


const CURRENT_READERS_THRESHOLD: usize = READERS / 3;
const MIN_DIFFERENT_READS: usize = if cfg!(miri) { 100 } else { 200 };
const READERS: usize = 10;
const WRITERS: usize = 5;
