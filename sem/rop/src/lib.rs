#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

#![allow(named_asm_labels)]


use std::{arch::asm, vec, vec::Vec};


pub mod ropchain;


pub fn gadgets() -> Vec<usize> {
    let mut gadgets = vec![0; 5];

    unsafe {
        asm!(
            "
            mov qword ptr [rdi     ], OFFSET gadget0
            mov qword ptr [rdi +  8], OFFSET gadget1
            mov qword ptr [rdi + 16], OFFSET gadget2
            mov qword ptr [rdi + 24], OFFSET gadget3
            mov qword ptr [rdi + 32], OFFSET gadget4

            jmp gadgets_end

        gadget0:
            mov [rsp + 8], rdi
            ret

        gadget1:
            pop rax
            ret

        gadget2:
            add rdi, 1
            ret

        gadget3:
            shl rdi, 1
            ret

        gadget4:
            xor rdi, rdi
            ret

        gadgets_end:
            ",
            in("rdi") gadgets.as_mut_ptr(),
        );
    }

    gadgets
}


pub fn run_ropchain(ropchain: &[usize]) -> i64 {
    let result;

    unsafe {
        asm!(
            "
            mov r15, rsp

            mov qword ptr [rdi + 8 * rsi], OFFSET return_from_ropchain
            mov rsp, rdi
            ret

        return_from_ropchain:

            mov rsp, r15
            ",
            in("rsi") ropchain.len(),
            in("rdi") ropchain.as_ptr(),
            lateout("rax") result,
        );
    }

    result
}
