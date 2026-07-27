use core::arch::asm;

use rand::{distributions::Uniform, rngs::SmallRng, Rng, SeedableRng};

use asm::{sum, sum_array, sum_struct, sum_va_arg, Struct};


#[test]
fn test_sum() {
    check_signature(sum);

    assert_eq!(dressed_sum(1, 2), 3);
    assert_eq!(dressed_sum(10, 20), 30);
    assert_eq!(dressed_sum(i64::MIN, i64::MAX), -1);

    for x in -100..100 {
        for y in -100..100 {
            assert_eq!(dressed_sum(x, y), x + y);
        }
    }

    fn dressed_sum(x: i64, y: i64) -> i64 {
        let result;

        unsafe {
            asm!(
                "
                call sum
                ",
                in("rsi") x,
                in("rdi") y,
                lateout("rax") result,
            );
        }

        result
    }
}


#[test]
fn test_sum_struct() {
    check_signature(sum_struct);

    assert_eq!(dressed_sum(1, 2), 3);
    assert_eq!(dressed_sum(10, 20), 30);
    assert_eq!(dressed_sum(i64::MIN, i64::MAX), -1);

    for x in -100..100 {
        for y in -100..100 {
            assert_eq!(dressed_sum(x, y), x + y);
        }
    }

    fn dressed_sum(x: i64, y: i64) -> i64 {
        let mut s = Struct { x, y, result: 0 };

        unsafe {
            asm!(
                "
                call sum_struct
                ",
                in("rdi") &mut s,
            );
        }

        s.result
    }
}


#[test]
fn test_sum_array() {
    check_signature(sum_array);

    assert_eq!(dressed_sum(&[1, 2]), 3);
    assert_eq!(dressed_sum(&[10, 20]), 30);
    assert_eq!(dressed_sum(&[i64::MIN, i64::MAX]), -1);

    for x in -100..100 {
        for y in -100..100 {
            for z in -100..100 {
                assert_eq!(dressed_sum(&[x, y, z]), x + y + z);
            }
        }
    }

    let mut rng = SmallRng::seed_from_u64(SEED);
    let range = Uniform::new(-100, 100);

    for _iteration in 0..1000 {
        for size in 0..100 {
            let x: Vec<_> = (0..size).map(|_| rng.sample(&range)).collect();
            assert_eq!(dressed_sum(&x), x.iter().sum());
        }
    }

    fn dressed_sum(x: &[i64]) -> i64 {
        let result;

        unsafe {
            asm!(
                "
                call sum_array
                ",
                in("rdi") x.as_ptr(),
                in("rsi") x.len(),
                lateout("rax") result,
            );
        }

        result
    }
}


#[test]
fn test_sum_va_arg() {
    check_signature(sum_va_arg);

    assert_eq!(dressed_sum(&[1, 2]), 3);
    assert_eq!(dressed_sum(&[10, 20]), 30);
    assert_eq!(dressed_sum(&[i64::MIN, i64::MAX]), -1);

    for x in -100..100 {
        for y in -100..100 {
            for z in -100..100 {
                assert_eq!(dressed_sum(&[x, y, z]), x + y + z);
            }
        }
    }

    let mut rng = SmallRng::seed_from_u64(SEED);
    let range = Uniform::new(-100, 100);

    for _iteration in 0..1000 {
        for size in 0..100 {
            let x: Vec<_> = (0..size).map(|_| rng.sample(&range)).collect();
            assert_eq!(dressed_sum(&x), x.iter().sum());
        }
    }

    /// Переключает стек внутрь `x`.
    /// Регистр `R15` зарезервирован под сохранённый `RSP`.
    fn dressed_sum(x: &[i64]) -> i64 {
        let result;

        unsafe {
            asm!(
                "
                mov r15, rsp

            2:
                or rcx, rcx
                jz 3f
                dec rcx
                push [rsi + 8 * rcx]
                jmp 2b
            3:

                xor rsi, rsi

                call sum_va_arg

                mov rsp, r15
                ",
                in("rcx") x.len(),
                in("rsi") x.as_ptr(),
                in("rdi") x.len(),
                lateout("rax") result,
            );
        }

        result
    }
}


fn check_signature(_function: extern "C" fn() -> !) {
}


const SEED: u64 = 314159265;
