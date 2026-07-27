#![deny(warnings)]


use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use derive_more::{Add, Sum};
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

    let run = Arc::new(AtomicBool::new(true));
    let point = Arc::new(AtomicCorrelationPoint::new());

    let threads: Vec<_> = (0..THREAD_COUNT)
        .map(|thread| {
            let point = point.clone();

            thread::Builder::new()
                .name(format!("reader #{}", thread))
                .spawn(move || reader(&point))
                .unwrap()
        })
        .collect();

    let writer_thread = {
        let run = run.clone();

        thread::Builder::new()
            .name("writer".to_string())
            .spawn(move || writer(&point, &run))
            .unwrap()
    };

    let stats = threads
        .into_iter()
        .map(|thread| thread.join().expect("readers should finish successfully"))
        .sum();
    let Stats {
        consistent,
        inconsistent,
    } = stats;

    run.store(false, Ordering::Release);

    writer_thread.join().expect("writer should finish successfully");

    debug!(consistent, inconsistent);
    assert!(
        consistent >= THREAD_COUNT * MIN_DIFFERENT_READS,
        "detected only {} consistent data reads",
        consistent,
    );
    assert_eq!(
        inconsistent, 0,
        "detected {} inconsistent data reads",
        inconsistent,
    );
}


fn reader(point: &AtomicCorrelationPoint) -> Stats {
    let mut iteration = 0;
    let mut prev = 0;
    let mut stats = Stats::default();
    let mut report = true;

    while iteration < MIN_DIFFERENT_READS {
        let data = point.load();

        if 2 * data.count() == data.tsc() {
            if data.count() > 0 {
                stats.consistent += 1;
            }
        } else {
            if report {
                error!(?data, iteration, "inconsistent data");
                report = false;
            }
            stats.inconsistent += 1;
        }

        if data.count() != prev {
            prev = data.count();
            iteration += 1;
        }
    }

    stats
}


fn writer(point: &AtomicCorrelationPoint, run: &AtomicBool) {
    let mut value = 1;

    while run.load(Ordering::Acquire) {
        point.store(time::test_scaffolding::new_point(value, 2 * value));
        point.inc(2 * value + 2);
        value += 3;
    }
}


#[derive(Add, Clone, Copy, Default, Sum)]
struct Stats {
    consistent: usize,
    inconsistent: usize,
}


const MIN_DIFFERENT_READS: usize = 1_000;
const THREAD_COUNT: usize = 10;
