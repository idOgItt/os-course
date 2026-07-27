#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

//! Библиотека для пользовательских процессов.

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![deny(warnings)]
#![feature(alloc_error_handler)]
#![feature(allocator_api)]
#![feature(naked_functions)]
#![feature(slice_ptr_get)]
#![no_std]
#![warn(clippy::missing_docs_in_private_items)]
#![warn(missing_docs)]


/// Аллокатор памяти общего назначения в пространстве пользователя, реализованный через
/// системные вызовы [`syscall::map()`], [`syscall::unmap()`] и [`syscall::copy_mapping()`].
pub mod allocator;

/// Вспомогательные функции для работы с виртуальными страницами
/// [`memory::copy_page`] и [`memory::temp_page()`],
/// а также с таблицами страниц [`memory::page_table()`].
pub mod memory;

/// Системные вызовы.
pub mod syscall;


extern crate alloc;

use core::{
    fmt::{Error, Result, Write},
    mem,
    panic::PanicInfo,
    ptr,
    sync::atomic::{AtomicPtr, Ordering},
};

use static_assertions::const_assert;
use tracing_core::{dispatch, dispatch::Dispatch, Level};

use ku::{
    backtrace::Backtrace,
    info,
    info::ProcessInfo,
    log::{error, LOG_COLLECTOR},
    process::ExitCode,
    sync,
};


/// Точка входа в процесс пользователя.
/// Получает от ядра pid процесса и ссылку `process_info` на информацию о текущем процессе.
#[unsafe(no_mangle)]
pub extern "C" fn _start(_pid: usize, process_info: &'static mut ProcessInfo) -> ! {
    info::set_process_info(process_info);

    LOG_COLLECTOR.set_flush(syscall::sched_yield);

    dispatch::set_global_default(Dispatch::from_static(&LOG_COLLECTOR)).unwrap();

    unsafe extern "Rust" {
        fn main();
    }

    unsafe {
        main();
    }

    syscall::exit(ExitCode::Ok.into());
}


/// Запоминает `panic_handler` для последующего вызова в случае паники.
pub fn set_panic_handler(panic_handler: fn(&PanicInfo)) {
    PANIC_HANDLER.store(panic_handler as *mut _, Ordering::Relaxed);
}


/// Обработчик паники.
#[cold]
#[inline(never)]
#[panic_handler]
fn panic(panic_info: &PanicInfo) -> ! {
    sync::start_panicing();

    if LOG_COLLECTOR.is_buzy() {
        let mut flight_recorder = FlightRecorder;
        let _ = flight_recorder.write_fmt(format_args!("{}", panic_info.message()));
        if let Some(location) = panic_info.location() {
            let _ = syscall::log_value(
                Level::ERROR,
                location.file(),
                location.line().try_into().expect("u32 should fit into usize"),
            );
        }
    }

    if let Ok(backtrace) = Backtrace::with_stack(ku::process_info().stack()) {
        error!(message = %panic_info, %backtrace);
    } else {
        error!(message = %panic_info);
    }

    let panic_handler = PANIC_HANDLER.load(Ordering::Relaxed);
    if !panic_handler.is_null() {
        unsafe {
            const_assert!(mem::size_of::<*const ()>() == mem::size_of::<fn(&PanicInfo)>());
            let panic_handler = mem::transmute::<*const (), fn(&PanicInfo)>(panic_handler);
            (panic_handler)(panic_info);
        }
    }

    syscall::exit(ExitCode::Panic.into());
}


/// Логгер для паник, потенциально возникших внутри стандарного [`LOG_COLLECTOR`].
struct FlightRecorder;


impl Write for FlightRecorder {
    fn write_str(&mut self, text: &str) -> Result {
        syscall::log_value(Level::ERROR, text, 0).map_err(|_| Error)
    }
}


/// Задаёт функцию `main()` пользовательского процесса.
#[macro_export]
macro_rules! entry {
    ($path:path) => {
        #[unsafe(export_name = "main")]
        pub unsafe fn check_main_signature() {
            let main: fn() = $path;

            main()
        }
    };
}


/// Адрес обработчика `panic_handler()`, установленный с помощью [`set_panic_handler()`].
static PANIC_HANDLER: AtomicPtr<()> = AtomicPtr::new(ptr::null_mut());
