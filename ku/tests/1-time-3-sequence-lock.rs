#![deny(warnings)]


use std::{
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    thread,
    time::Duration,
};

use rstest::rstest;

use ku::{
    log::{debug, error},
    sync::SequenceLock,
};


mod log;


#[rstest]
#[timeout(Duration::from_secs(1))]
fn write() {
    let sequence_lock = SequenceLock::new(0);

    unsafe {
        *sequence_lock.write() = 3;
    }

    let a = sequence_lock.read();
    let b = sequence_lock.read();

    assert_eq!(a, 3);
    assert_eq!(b, 3);
}


#[rstest]
#[timeout(Duration::from_secs(1))]
fn write_lock() {
    let sequence_lock = SequenceLock::new(0);

    *sequence_lock.write_lock() = 3;

    let a = sequence_lock.read();
    let b = sequence_lock.read();

    assert_eq!(a, 3);
    assert_eq!(b, 3);
}


#[rstest]
#[timeout(Duration::from_secs(1))]
fn exclusive_access() {
    let mut sequence_lock = SequenceLock::new(0);

    *sequence_lock.get_mut() = 3;

    let a = sequence_lock.read();
    let b = sequence_lock.read();

    assert_eq!(a, 3);
    assert_eq!(b, 3);
}


#[rstest]
#[timeout(Duration::from_secs(60))]
fn single_writer() {
    log::init();

    static CONSISTENT: AtomicUsize = AtomicUsize::new(0);
    static INCONSISTENT: AtomicUsize = AtomicUsize::new(0);
    static RUN: AtomicBool = AtomicBool::new(true);
    static SEQUENCE_LOCK: SequenceLock<(usize, usize)> = SequenceLock::new((0, 0));

    let mut threads = Vec::new();

    for thread in 0..THREAD_COUNT {
        threads.push(
            thread::Builder::new()
                .name(format!("reader_thread #{}", thread))
                .spawn(move || {
                    reader(&SEQUENCE_LOCK, &CONSISTENT, &INCONSISTENT);
                })
                .unwrap(),
        );
    }

    let writer_thread = thread::Builder::new()
        .name("writer_thread".to_string())
        .spawn(move || {
            exclusive_writer(&SEQUENCE_LOCK, &RUN);
        })
        .unwrap();

    while let Some(thread) = threads.pop() {
        assert!(thread.join().is_ok());
    }

    RUN.store(false, Ordering::Release);

    assert!(writer_thread.join().is_ok());

    let consistent_count = CONSISTENT.load(Ordering::Relaxed);
    let inconsistent_count = INCONSISTENT.load(Ordering::Relaxed);
    debug!(consistent_count, inconsistent_count);
    assert!(
        consistent_count >= THREAD_COUNT * MIN_DIFFERENT_READS,
        "detected only {} consistent data reads",
        consistent_count,
    );
    assert_eq!(
        inconsistent_count, 0,
        "detected {} inconsistent data reads",
        inconsistent_count,
    );
}


#[rstest]
#[timeout(Duration::from_secs(60))]
fn multiple_writers() {
    log::init();

    static CONSISTENT: AtomicUsize = AtomicUsize::new(0);
    static INCONSISTENT: AtomicUsize = AtomicUsize::new(0);
    static RUN: AtomicBool = AtomicBool::new(true);
    static SEQUENCE_LOCK: SequenceLock<(usize, usize)> = SequenceLock::new((0, 0));

    let mut reader_threads = Vec::new();
    let mut writer_threads = Vec::new();

    for thread in 0..THREAD_COUNT {
        reader_threads.push(
            thread::Builder::new()
                .name(format!("reader_thread #{}", thread))
                .spawn(move || {
                    reader(&SEQUENCE_LOCK, &CONSISTENT, &INCONSISTENT);
                })
                .unwrap(),
        );

        writer_threads.push(
            thread::Builder::new()
                .name(format!("writer_thread #{}", thread))
                .spawn(move || {
                    non_exclusive_writer(&SEQUENCE_LOCK, &RUN);
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
    debug!(consistent_count, inconsistent_count);
    assert!(
        consistent_count >= THREAD_COUNT * MIN_DIFFERENT_READS,
        "detected only {} consistent data reads",
        consistent_count,
    );
    assert_eq!(
        inconsistent_count, 0,
        "detected {} inconsistent data reads",
        inconsistent_count,
    );
}


#[rstest]
#[timeout(Duration::from_secs(60))]
fn multiple_exclusive_writers() {
    log::init();

    static RUN: AtomicBool = AtomicBool::new(true);
    static SEQUENCE_LOCK: SequenceLock<(usize, usize)> = SequenceLock::new((0, 0));

    let mut threads = Vec::new();

    for thread in 0..THREAD_COUNT {
        threads.push(
            thread::Builder::new()
                .name(format!("writer_thread #{}", thread))
                .spawn(move || {
                    exclusive_writer(&SEQUENCE_LOCK, &RUN);
                })
                .unwrap(),
        );
    }

    thread::sleep(Duration::from_secs(1));

    RUN.store(false, Ordering::Release);

    let mut non_exclusivity_detection_count = 0;
    while let Some(thread) = threads.pop() {
        if thread.join().is_err() {
            non_exclusivity_detection_count += 1;
        }
    }
    debug!(non_exclusivity_detection_count);
    assert!(non_exclusivity_detection_count > 0);
}


fn reader(
    sequence_lock: &SequenceLock<(usize, usize)>,
    global_consistent: &AtomicUsize,
    global_inconsistent: &AtomicUsize,
) {
    let mut consistent = 0;
    let mut inconsistent = 0;
    let mut iteration = 0;
    let mut prev = 0;
    let mut report = true;

    while iteration < MIN_DIFFERENT_READS {
        let data = sequence_lock.read();

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

        if data.0 != prev {
            prev = data.0;
            iteration += 1;
        }
    }

    global_consistent.fetch_add(consistent, Ordering::Relaxed);
    global_inconsistent.fetch_add(inconsistent, Ordering::Relaxed);
}


fn exclusive_writer(sequence_lock: &SequenceLock<(usize, usize)>, run: &AtomicBool) {
    while run.load(Ordering::Acquire) {
        let mut lock = unsafe { sequence_lock.write() };
        let data = *lock;

        let i = data.0;
        lock.0 = i + 1;

        thread::yield_now();

        lock.1 = 2 * i + 2;
    }
}


fn non_exclusive_writer(sequence_lock: &SequenceLock<(usize, usize)>, run: &AtomicBool) {
    while run.load(Ordering::Acquire) {
        let mut lock = sequence_lock.write_lock();
        let data = *lock;

        let i = data.0;
        lock.0 = i + 1;

        thread::yield_now();

        lock.1 = 2 * i + 2;
    }
}


const MIN_DIFFERENT_READS: usize = 100;
const THREAD_COUNT: usize = 10;
