#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

#![deny(warnings)]
#![no_main]
#![no_std]


use core::arch::asm;

use lib::entry;


entry!(main);


#[allow(named_asm_labels)]
fn main() {
    unsafe {
        asm!(
            "
            // Test that the kernel stores user-mode context properly.
            mov rax, 77701
            mov rbx, 77702
            mov rcx, 77703
            mov rdx, 77704
            mov rdi, 77705
            mov rsi, 77706
            mov rbp, 77707
            mov r8, 77708
            mov r9, 77709
            mov r10, 77710
            mov r11, 77711
            mov r12, 77712
            mov r13, 77713
            mov r14, 77714
            mov r15, 77715

            loop:
            jmp loop
            "
        );
    }
}
