#![feature(strict_provenance)]


use backtrace::{backtrace, Backtrace};


macro_rules! check_ptr {
    ($from:expr, $ptr:expr, $to:expr) => {
        let from = $from as *const ();
        let to = $to as *const ();
        eprintln!(
            "checking {} <= {} < {}: {:#X} in [{:#X}, {:#X})",
            stringify!($from),
            stringify!($ptr),
            stringify!($to),
            $ptr.addr(),
            from.addr(),
            to.addr(),
        );
        assert!((from.addr()..to.addr()).contains(&$ptr.addr()));
    };
}


struct Bt(Backtrace, Backtrace);


fn r(recursion: usize, bt: &mut Bt) {
    r0(recursion, bt);
    r1(recursion, bt);
}


fn r0(recursion: usize, bt: &mut Bt) {
    if recursion > 1 {
        r0(recursion - 1, bt);
    } else {
        bt.0 = backtrace();
    }
}


fn r1(recursion: usize, bt: &mut Bt) {
    if recursion > 1 {
        r1(recursion - 1, bt);
    } else {
        bt.1 = backtrace();
    }
}


fn s3(bt: &mut Backtrace) {
    *bt = backtrace();
}


fn s2(bt: &mut Backtrace) {
    s3(bt);
}


fn s1(bt: &mut Backtrace) {
    s2(bt);
}


/// Если падает этот тест, то проблема в тестах, а не в решении задачи.
#[test]
fn check_test_implementation() {
    check_ptr!(r, r0 as *const (), r1);
    check_ptr!(r1, s3 as *const (), s2);
    check_ptr!(s2, s1 as *const (), simple);
}


#[test]
fn simple() {
    let mut bt = Backtrace::new();

    s1(&mut bt);

    check_ptr!(s3, bt[0], s2);
    check_ptr!(s2, bt[1], s1);
    check_ptr!(s1, bt[2], simple);
}


#[test]
fn recursive() {
    let mut bt = Bt(Backtrace::new(), Backtrace::new());
    let mut min_len = 1;

    for recursion in (1..20).step_by(5) {
        r(recursion, &mut bt);

        if recursion == 1 {
            min_len = bt.0.len();
        }

        assert_eq!(bt.0.len(), min_len + recursion - 1);
        assert_eq!(bt.1.len(), min_len + recursion - 1);

        for i in 0..recursion {
            check_ptr!(r0, bt.0[i], r1);
        }
        check_ptr!(r, bt.0[recursion], r0);

        for i in 0..recursion {
            check_ptr!(r1, bt.1[i], s3);
        }
        check_ptr!(r, bt.1[recursion], r0);
    }
}
