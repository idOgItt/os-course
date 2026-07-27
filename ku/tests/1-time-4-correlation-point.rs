#![deny(warnings)]


use std::{
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    thread,
    time::Duration,
};

use rstest::rstest;

use ku::{
    log::{debug, error},
    time::{self, test_scaffolding::AtomicCorrelationPoint, CorrelationPoint},
};


mod log;


#[rstest]
#[timeout(Duration::from_secs(1))]
fn correlation_point() {
    let x = CorrelationPoint::now(7);

    assert!(x.is_valid());
    assert_eq!(x.count(), 7);
    assert!(x.tsc() > 0);
}


#[rstest]
#[timeout(Duration::from_secs(1))]
fn atomic_correlation_point() {
    let x = AtomicCorrelationPoint::new();
    assert!(!x.is_valid());

    x.inc(3);
    assert!(x.is_valid());
    assert_eq!(x.load().count(), 1);
    assert_eq!(x.load().tsc(), 3);

    let point = time::test_scaffolding::new_point(4, 5);
    x.store(point);
    assert_eq!(x.load(), point);
}


#[rstest]
#[timeout(Duration::from_secs(60))]
fn single_writer() {
    log::init();

    static CONSISTENT: AtomicUsize = AtomicUsize::new(0);
    static INCONSISTENT: AtomicUsize = AtomicUsize::new(0);
    static RUN: AtomicBool = AtomicBool::new(true);
    static POINT: AtomicCorrelationPoint = AtomicCorrelationPoint::new();

    let mut threads = Vec::new();

    for thread in 0..THREAD_COUNT {
        threads.push(
            thread::Builder::new()
                .name(format!("reader_thread #{}", thread))
                .spawn(move || {
                    reader(&POINT, &CONSISTENT, &INCONSISTENT);
                })
                .unwrap(),
        );
    }

    let writer_thread = thread::Builder::new()
        .name("writer_thread".to_string())
        .spawn(move || {
            writer(&POINT, &RUN);
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


fn reader(
    point: &AtomicCorrelationPoint,
    global_consistent: &AtomicUsize,
    global_inconsistent: &AtomicUsize,
) {
    let mut consistent = 0;
    let mut inconsistent = 0;
    let mut iteration = 0;
    let mut prev = 0;
    let mut report = true;

    while iteration < MIN_DIFFERENT_READS {
        let data = point.load();

        if 2 * data.count() == data.tsc() {
            if data.count() > 0 {
                consistent += 1;
            }
        } else {
            if report {
                error!(?data, iteration, "inconsistent data");
                report = false;
            }
            inconsistent += 1;
        }

        if data.count() != prev {
            prev = data.count();
            iteration += 1;
        }
    }

    global_consistent.fetch_add(consistent, Ordering::Relaxed);
    global_inconsistent.fetch_add(inconsistent, Ordering::Relaxed);
}


fn writer(point: &AtomicCorrelationPoint, run: &AtomicBool) {
    let mut value = 1;

    while run.load(Ordering::Acquire) {
        point.store(time::test_scaffolding::new_point(value, 2 * value));
        point.inc(2 * value + 2);
        value += 3;
    }
}


const MIN_DIFFERENT_READS: usize = 1_000;
const THREAD_COUNT: usize = 10;
