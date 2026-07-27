#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

#![feature(naked_functions)]


use core::arch::asm;


/// Складывает регистры RSI и RDI, а результат возвращает в RAX.
#[naked]
#[unsafe(no_mangle)]
pub extern "C" fn sum() -> ! {
    unsafe {
        asm!(
            "add rsi, rdi",
            "mov rax, rsi",
            "ret",
            options(noreturn),
        )
    }
}


#[repr(C)]
pub struct Struct {
    pub x: i64,
    pub y: i64,
    pub result: i64,
}


/// Складывает поля структуры.
///
/// Принимает указатель на структуру типа [`Struct`] в регистре RDI.
/// Записывает в её поле `result` значение `x + y`.
#[naked]
#[unsafe(no_mangle)]
pub extern "C" fn sum_struct() -> ! {
    unsafe {
        asm!(
            "mov rax, [rdi]",
            "add rax, [rdi + 8]",
            "mov [rdi + 16], rax",
            "ret",
            options(noreturn),
        )
    }
}


/// Складывает элементы массива.
///
/// Принимает указатель на массив 64-битных целых
/// чисел в регистре RDI и количество элементов в массиве
/// в регистре RSI.
/// Возвращает сумму элементов массива в регистре RAX.
#[allow(named_asm_labels)]
#[naked]
#[unsafe(no_mangle)]
pub extern "C" fn sum_array() -> ! {
    unsafe {
        asm!(
            "xor rax, rax",
            "xor rcx, rcx",
            "while_f:",
            "cmp rcx, rsi",
            "jge while_e",
            "add rax, [rdi + 8 * rcx]",
            "inc rcx",
            "jmp while_f",
            "while_e:",
            "ret",
            options(noreturn),
        )
    }
}


/// Складывает переменное количество аргументов.
///
/// Принимает количество 64-битных целых чисел в регистре RDI.
/// Сами элементы лежат на стеке сразу после адреса возврата из функции.
/// Возвращает сумму элементов массива в регистре RAX.
/// Не меняет содержимое регистра R15.
#[allow(named_asm_labels)]
#[naked]
#[unsafe(no_mangle)]
pub extern "C" fn sum_va_arg() -> ! {
    unsafe {
        asm!(
            "xor rcx, rcx",
            "xor rax, rax",
            "while_fs:",
            "cmp rcx, rdi",
            "jge while_en",
            "add rax, [rsp + 8 + 8 * rcx]",
            "inc rcx",
            "jmp while_fs",
            "while_en:",
            "ret",
            options(noreturn),
        )
    }
}
