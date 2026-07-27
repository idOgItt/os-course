#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

//! Общая для пространств ядра и пользователя библиотека.
//! ku --- **k**ernel && **u**ser.

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![deny(warnings)]
#![feature(adt_const_params)]
#![feature(allocator_api)]
#![feature(atomic_from_mut)]
#![feature(cell_update)]
#![feature(gen_blocks)]
#![feature(let_chains)]
#![feature(maybe_uninit_fill, maybe_uninit_slice)]
#![feature(ptr_as_uninit)]
#![feature(slice_ptr_get)]
#![feature(step_trait)]
#![feature(strict_provenance)]
#![feature(trait_alias)]
#![no_std]
#![warn(clippy::missing_docs_in_private_items)]
#![warn(missing_docs)]


extern crate alloc;
extern crate rlibc;


/// Аллокаторы памяти общего назначения.
pub mod allocator;

/// Поддержка печати бектрейсов.
pub mod backtrace;

/// Перечисление для возможных ошибок [`Error`] и соответствующий [`Result`].
pub mod error;

/// Информации о системе, доступная пользовательским процессам.
pub mod info;

/// Поддержка логирования макросами библиотеки [`tracing`].
///
/// Сериализует сообщения в [`ring_buffer`] и
/// передаёт их для логирования в ядро через [`ProcessInfo::log()`].
/// Сериализация осуществляется с помощью [`serde`] в формате [`postcard`].
pub mod log;

/// LRU--кеш
/// ([Least Recently Used](https://en.wikipedia.org/wiki/Cache_replacement_policies#LRU)) ---
/// кеш с реализацией алгоритма вытеснения давно неиспользуемых данных.
pub mod lru;

/// Здесь собраны базовые примитивы для работы с памятью,
/// которые нужны и в ядре, и в пространстве пользователя.
pub mod memory;

/// Здесь собраны функции и структуры для работы с процессами,
/// которые нужны и в ядре, и в пространстве пользователя.
pub mod process;

/// [Непрерывный циклический буфер](https://fgiesen.wordpress.com/2012/07/21/the-magic-ring-buffer/).
pub mod ring_buffer;

/// Примитивы синхронизации [`Spinlock`] и [`SequenceLock`].
pub mod sync;

/// Здесь собраны базовые примитивы для работы со временем,
/// которые нужны и в ядре, и в пространстве пользователя.
pub mod time;


use x86_64::instructions::{self, interrupts};

pub use error::{Error, Result};
pub use info::{
    process_info,
    set_process_info,
    set_system_info,
    system_info,
    ProcessInfo,
    SystemInfo,
};
pub use ring_buffer::{ReadBuffer, RingBufferReadTx, RingBufferWriteTx, WriteBuffer};
pub use sync::{sequence_lock::SequenceLock, spinlock::Spinlock};
pub use time::{delay, now, now_ms, timer, tsc, Hz, Tsc, TscDuration};


/// Останавливает процессор инструкцией
/// [`hlt`](https://www.felixcloutier.com/x86/hlt)
/// при запрещённых внешних прерываниях.
///
/// # Safety
///
/// Требует привилегированного режима работы.
#[cold]
#[inline(never)]
pub unsafe fn halt() -> ! {
    loop {
        interrupts::without_interrupts(instructions::hlt)
    }
}
