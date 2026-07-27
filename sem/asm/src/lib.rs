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
            "
            // TODO: your code here.
            ",
            options(noreturn),
        );
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
            "
            // TODO: your code here.
            ",
            options(noreturn),
        );
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
            "
            // TODO: your code here.
            ",
            options(noreturn),
        );
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
            "
            // TODO: your code here.
            ",
            options(noreturn),
        );
    }
}
