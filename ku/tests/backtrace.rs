#![deny(warnings)]


use ku::{backtrace::Backtrace, memory::Virt};

use macros::with_sentinel_frame;


#[derive(PartialEq, Eq, Debug, Default)]
struct BacktraceStats {
    backtrace_depth: usize,
    found_sentinel: bool,
    stopped_by_sentinel: bool,
}


fn run_at_depth<T, Arg, F: FnOnce(Arg) -> T>(depth: usize, f: F, arg: Arg) -> T {
    if depth == 0 {
        f(arg)
    } else {
        run_at_depth(depth - 1, f, arg)
    }
}


fn capture_backtrace_fingerprint(backtrace_stats: &mut BacktraceStats) {
    if let Ok(bt) = Backtrace::current() {
        for frame in bt {
            if backtrace_stats.found_sentinel {
                backtrace_stats.stopped_by_sentinel = false;
                break;
            }
            backtrace_stats.backtrace_depth += 1;
            if frame.return_address() == Virt::zero() {
                backtrace_stats.found_sentinel = true;
                backtrace_stats.stopped_by_sentinel = true;
            }
        }
    }
}


#[with_sentinel_frame]
fn write_backtrace_stats_to(depth: usize, backtrace_stats: &mut BacktraceStats) {
    run_at_depth(depth, capture_backtrace_fingerprint, backtrace_stats)
}


fn get_backtrace_stats(depth: usize) -> BacktraceStats {
    let mut stats = BacktraceStats::default();
    write_backtrace_stats_to(depth, &mut stats);
    stats
}


#[test]
fn backtraces() {
    for backtrace_depth in 0..10 {
        let target_stats = get_backtrace_stats(backtrace_depth);

        assert!(target_stats.found_sentinel);
        assert!(target_stats.stopped_by_sentinel);
        assert!(target_stats.backtrace_depth >= backtrace_depth);
        assert!(target_stats.backtrace_depth < backtrace_depth + 7);

        for run_depth in 0..5 {
            assert_eq!(
                run_at_depth(run_depth, get_backtrace_stats, backtrace_depth),
                target_stats,
            );
        }
    }
}
