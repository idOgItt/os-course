use alloc::vec::Vec;
use core::{arch::asm, str};

use x86_64::registers::model_specific::{Efer, EferFlags, LStar, SFMask, Star};

use ku::{
    log::{self, event, Level},
    process::{ExitCode, MiniContext, RFlags, ResultCode, State, Syscall, SyscallResult},
    sync::spinlock::SpinlockGuard,
};

use crate::{
    allocator::MemoryAllocator,
    error::{
        Error::{InvalidArgument, NoPage, Overflow, PermissionDenied, WrongAlignment},
        Result,
    },
    gdt::Gdt,
    log::{debug, error, info, trace, warn},
    memory::{
        self,
        mmu::{PageTableEntry, PageTableFlags},
        Block,
        Frame,
        Page,
        Virt,
        FRAME_ALLOCATOR,
        KERNEL_RW,
        USER,
        USER_READ,
    },
    smp::{Cpu, KERNEL_RSP_OFFSET_IN_CPU},
};

use super::{process::TrapContext, Pid, Process, Scheduler, Table};

use lock_set::{lock_dst, lock_src_dst};

// Used in docs.
#[allow(unused)]
use crate::error::Error;


/// Инициализация системных вызовов.
/// Подготавливает процессор к выполнению инструкций
/// [syscall](https://www.felixcloutier.com/x86/syscall) и
/// [sysret](https://www.felixcloutier.com/x86/sysret).
pub(crate) fn init() {
    let syscall_virt = Virt::from_ptr(syscall_trampoline as *const ());
    debug!(%syscall_virt);

    // TODO: your code here.
}


/// Получает управление при выполнении инструкции
/// [syscall](https://www.felixcloutier.com/x86/syscall).
///
/// Переключает стек на стек ядра, разрешает прерывания и
/// передаёт управление в функцию [`syscall()`].
#[naked]
extern "C" fn syscall_trampoline() -> ! {
    unsafe {
        asm!(
            "
            // TODO: your code here.
            ",

            // TODO: your code here.
            options(noreturn),
        );
    }
}


/// Выполняет диспетчеризацию системных вызовов по аргументу `number` --- номеру системного вызова.
///
/// Передаёт в функции, реализующие конкретные системные вызовы,
/// нужную часть аргументов `arg0`--`arg4`.
/// После выполнения функции конкретного системного вызова,
/// с помощью функции [`sysret()`] возвращает управление в контекст пользователя,
/// задаваемый `rip` и `rsp`.
#[unsafe(no_mangle)]
extern "C" fn syscall(
    // https://wiki.osdev.org/System_V_ABI#x86-64:
    // Parameters to functions are passed in the registers
    // rdi, rsi, rdx, rcx, r8, r9,
    // and further values are passed on the stack in reverse order.
    number: usize, // rdi
    arg0: usize,   // rsi
    arg1: usize,   // rdx
    rip: Virt,     // rcx
    rsp: Virt,     // r8
    arg2: usize,   // r9
    // Stack, push in reverse order.
    arg3: usize,
    arg4: usize,
) -> ! {
    assert!(
        RFlags::read().contains(RFlags::INTERRUPT_FLAG),
        "enable the interrupts during the system calls",
    );

    // TODO: your code here.
    unimplemented!();
}


/// С помощью инструкции [sysret](https://www.felixcloutier.com/x86/sysret)
/// возвращает управление в контекст пользователя `context`.
///
/// Передаёт пользователю результат системного вызова в виде кода успеха или ошибки `result` и
/// полезной нагрузки `res`.
fn sysret(context: MiniContext, result: ResultCode, res: SyscallResult) -> ! {
    trace!(%context, ?result, ?res, "sysret");

    // TODO: your code here.

    unsafe {
        asm!(
            "
            // TODO: your code here.
            ",

            // TODO: your code here.

            options(noreturn),
        );
    }
}


/// Выполняет системный вызов
/// [`lib::syscall::exit(code)`](https://sergey-v-galtsev.github.io/labs-description/doc/lib/syscall/fn.exit.html).
///
/// Освобождает слот таблицы процессов и возвращается в контекст ядра,
/// из которого пользовательский процесс был запущен.
fn exit(process: SpinlockGuard<Process>, code: usize) -> ! {
    // TODO: your code here.
    unimplemented!();
}


/// Выполняет системный вызов
/// [`lib::syscall::log_value(message, value)`](https://sergey-v-galtsev.github.io/labs-description/doc/lib/syscall/fn.log_value.html).
///
/// Логирует строку `message` типа `&str`, заданную началом `start` и длиной `len`,
/// а также число `value`.
#[allow(unused_mut)] // TODO: remove before flight.
fn log_value(
    mut process: SpinlockGuard<Process>,
    level: usize,
    start: usize,
    len: usize,
    value: usize,
) -> Result<SyscallResult> {
    // TODO: your code here.
    unimplemented!();
}


/// Выполняет системный вызов
/// [`lib::syscall::sched_yield()`](https://sergey-v-galtsev.github.io/labs-description/doc/lib/syscall/fn.sched_yield.html).
///
/// Перепланирует процесс в конец очереди готовых к исполнению процессов и
/// забирает у него CPU функцией [`Process::sched_yield()`],
/// которая вернёт управление в другой контекст ядра ---
/// в контекст из которого была вызвана функция [`Process::enter_user_mode()`].
/// Текущий контекст исходного процесса --- `context` --- записывает в него,
/// чтобы в дальнейшем в него можно было вернуться через [`Process::enter_user_mode()`].
#[allow(unused_mut)] // TODO: remove before flight.
fn sched_yield(mut process: SpinlockGuard<Process>, context: MiniContext) -> ! {
    // TODO: your code here.
    unimplemented!();
}


/// Выполняет системный вызов
/// [`lib::syscall::exofork()`](https://sergey-v-galtsev.github.io/labs-description/doc/lib/syscall/fn.exofork.html).
///
/// Создаёт копию вызывающего процесса `process` и возвращает исходному процессу [`Pid`] копии.
/// Внутри копии возвращает [`Pid::Current`].
/// При этом новый процесс создаётся практически без адресного пространства и не готовый к работе.
/// Поэтому он, в частности, не ставится в очередь планировщика.
/// Текущий контекст исходного процесса --- `context` --- записывает в копию, чтобы в копии
/// вернуться туда же, куда происходит возврат из системного вызова для вызывающего процесса.
#[allow(unused_mut)] // TODO: remove before flight.
fn exofork(mut process: SpinlockGuard<Process>, context: MiniContext) -> Result<SyscallResult> {
    // TODO: your code here.
    unimplemented!();
}


/// Выполняет системный вызов
/// [`lib::syscall::map(dst_pid, dst_block, flags)`](https://sergey-v-galtsev.github.io/labs-description/doc/lib/syscall/fn.map.html).
///
/// Отображает в памяти процесса, заданного `dst_pid`, блок страниц размера `dst_size` байт
/// начиная с виртуального адреса `dst_address` с флагами доступа `flags`.
/// Если `dst_address` равен нулю,
/// сам выбирает свободный участок адресного пространства размера `dst_size`.
fn map(
    process: SpinlockGuard<Process>,
    dst_pid: usize,
    dst_address: usize,
    dst_size: usize,
    flags: usize,
) -> Result<SyscallResult> {
    // TODO: your code here.
    unimplemented!();
}


/// Выполняет системный вызов
/// [`lib::syscall::unmap(dst_pid, dst_block)`](https://sergey-v-galtsev.github.io/labs-description/doc/lib/syscall/fn.unmap.html).
///
/// Удаляет из виртуальной памяти целевого процесса `dst_pid` блок страниц
/// размера `dst_size` байт начиная с виртуального адреса `dst_address`.
fn unmap(
    process: SpinlockGuard<Process>,
    dst_pid: usize,
    dst_address: usize,
    dst_size: usize,
) -> Result<SyscallResult> {
    // TODO: your code here.
    unimplemented!();
}


/// Выполняет системный вызов
/// [`lib::syscall::copy_mapping(dst_pid, src_block, dst_block, flags)`](https://sergey-v-galtsev.github.io/labs-description/doc/lib/syscall/fn.copy_mapping.html).
///
/// Создаёт копию отображения виртуальной памяти из вызывающего процесса `process`
/// в процесс, заданный `dst_pid`.
/// Исходный диапазон начинается с виртуального адреса `src_address`,
/// целевой --- с виртуального адреса `dst_address`.
/// Размер диапазона --- `dst_size` байт.
///
/// В целевом процессе диапазон должен быть отображён с флагами:
///   - `flags`, если `flags != 0`.
///   - такими же, как в исходном диапазоне, если `flags == 0`.
///
/// Не допускает целевое отображение с более широким набором флагов, чем исходное.
/// После выполнения у процессов появляется область
/// [разделяемой памяти](https://en.wikipedia.org/wiki/Shared_memory).
fn copy_mapping(
    process: SpinlockGuard<Process>,
    dst_pid: usize,
    src_address: usize,
    dst_address: usize,
    dst_size: usize,
    flags: usize,
) -> Result<SyscallResult> {
    // TODO: your code here.
    unimplemented!();
}


/// Проверяет, что заданный блок виртуальных страниц `block` отображён в
/// адресное пространство процесса `process` с корректно заданными флагами `flags`.
/// Возвращает вектор физических фреймов, в которые отображены эти страницы.
/// См. также [`check_frame()`].
fn check_frames<'a>(
    process: &'a SpinlockGuard<Process>,
    block: Block<Page>,
    flags: PageTableFlags,
) -> Result<Vec<PageTableEntry, MemoryAllocator<'a>>> {
    // TODO: your code here.
    unimplemented!();
}


/// Выполняет отображение `src_ptes` в `dst_pages`
/// в адресное пространство процесса `process`.
///
/// В целевом процессе диапазон должен быть отображён с флагами:
///   - `flags`, если `flags` --- [`Some`].
///   - такими же, как в исходном диапазоне, если `flags` --- [`None`].
///
/// Количества элементов в `src_ptes` и `dst_pages` должны совпадать.
fn map_pages_to_frames(
    process: &SpinlockGuard<Process>,
    src_ptes: Vec<PageTableEntry, MemoryAllocator>,
    dst_pages: Block<Page>,
    flags: Option<PageTableFlags>,
) -> Result<()> {
    assert!(src_ptes.len() == dst_pages.count());

    // TODO: your code here.
    unimplemented!();
}


/// Выполняет системный вызов
/// [`lib::syscall::set_state(dst_pid, state)`](https://sergey-v-galtsev.github.io/labs-description/doc/lib/syscall/fn.set_state.html).
///
/// Переводит целевой процесс, заданный идентификатором `dst_pid`, в заданное состояние `state`.
/// Ставит его в очередь планировщика в случае [`State::Runnable`].
fn set_state(
    process: SpinlockGuard<Process>,
    dst_pid: usize,
    state: usize,
) -> Result<SyscallResult> {
    // TODO: your code here.
    unimplemented!();
}


/// Выполняет системный вызов
/// [`lib::syscall::set_trap_handler(dst_pid, trap_handler, trap_stack)`](https://sergey-v-galtsev.github.io/labs-description/doc/lib/syscall/fn.set_trap_handler.html).
///
/// Устанавливает для целевого процесса, заданного идентификатором `dst_pid`,
/// пользовательский обработчик прерывания с виртуальным адресом `rip` и стеком,
/// который задаётся блоком виртуальных адресов начиная с `stack_address` и размера `stack_size`.
/// Стек может быть не выровнен по границе страниц.
fn set_trap_handler(
    process: SpinlockGuard<Process>,
    dst_pid: usize,
    rip: usize,
    stack_address: usize,
    stack_size: usize,
) -> Result<SyscallResult> {
    // TODO: your code here.
    unimplemented!();
}


/// Проверяет, что `address` и `size` задают корректно выровненный диапазон страниц,
/// целиком лежащий внутри одной из
/// [двух непрерывных половин](https://en.wikipedia.org/wiki/X86-64#Virtual_address_space_details)
/// адресного пространства.
/// Возвращает блок соответствующих виртуальных страниц.
fn check_block(address: usize, size: usize) -> Result<Block<Page>> {
    // TODO: your code here.
    unimplemented!();
}


/// Проверяет, что заданная виртуальная страница `page` отображена в
/// адресное пространство процесса `process` с заданными флагами `flags`.
/// Флаги [`PageTableFlags::COPY_ON_WRITE`] и [`PageTableFlags::WRITABLE`]
/// при проверке считаются эквивалентными,
/// а флаг [`PageTableFlags::USER_ACCESSIBLE`] --- обязательно включённым.
/// Возвращает копию [`PageTableEntry`], с физическим фреймом,
/// в который она отображена, и флагами исходного отображения.
///
/// Возвращает ошибки:
///   - [`Error::NoPage`] если страница `page` не отображена.
///   - [`Error::PermissionDenied`] если страница отображена,
///     но не со всеми запрошенными флагами.
fn check_frame(
    process: &SpinlockGuard<Process>,
    page: Page,
    flags: PageTableFlags,
) -> Result<PageTableEntry> {
    // TODO: your code here.
    unimplemented!();
}


/// Проверяет, что `flags` задаёт валидный набор флагов отображения страниц пользователя.
///
/// Возвращает:
///   - [`None`], если `flags == 0`.
///   - Входные `flags` в виде [`PageTableFlags`], если `flags != 0`.
///
/// Возвращает ошибки:
///   - [`Error::InvalidArgument`], если во `flags` установлен бит,
///     не соответствующий никакому флагу [`PageTableFlags`].
///   - [`Error::PermissionDenied`], если `flags != 0` и
///     в них не включён [`PageTableFlags::USER_ACCESSIBLE`].
fn check_page_flags(flags: usize) -> Result<Option<PageTableFlags>> {
    // TODO: your code here.
    unimplemented!();
}


/// Работа с блокировкой одного процесса или парой блокировок двух разных процессов.
mod lock_set {
    use duplicate::duplicate_item;

    use ku::{
        error::{
            Error::{NoProcess, PermissionDenied},
            Result,
        },
        process::Pid,
        sync::spinlock::SpinlockGuard,
    };

    use super::super::{Process, Table};


    /// Проверяет, что процесс `src` имеет право модифицировать целевой процесс,
    /// заданный своим идентификатором `dst_pid`.
    /// Целевой процесс может совпадать с `src`.
    ///
    /// Модифицировать можно:
    ///   - Либо самого себя, задавая [`Pid::Current`] или явно собственный идентификатор [`Pid::Id`].
    ///   - Либо свой непосредственно дочерний процесс, задавая его идентификатор.
    ///
    /// Возвращает блокировку на процесс `dst`.
    pub(super) fn lock_dst(src: SpinlockGuard<Process>, dst_pid: usize) -> Result<LockSet> {
        LockSet::new(src, dst_pid, ProcessSet::Dst)
    }


    /// Проверяет, что процесс `src` имеет право модифицировать целевой процесс,
    /// заданный своим идентификатором `dst_pid`.
    /// Целевой процесс может совпадать с `src`.
    ///
    /// Модифицировать можно:
    ///   - Либо самого себя, задавая [`Pid::Current`] или явно собственный идентификатор [`Pid::Id`].
    ///   - Либо свой непосредственно дочерний процесс, задавая его идентификатор.
    ///
    /// Возвращает:
    ///   - Исходную блокировку на `src`, если `dst_pid` задаёт тот же процесс.
    ///   - Блокировки на процессы `src` и `dst`, если это разные процессы.
    ///
    /// Захватывает блокировки в правильном порядке для избежания
    /// [взаимоблокировки](https://en.wikipedia.org/wiki/Deadlock).
    pub(super) fn lock_src_dst(src: SpinlockGuard<Process>, dst_pid: usize) -> Result<LockSet> {
        LockSet::new(src, dst_pid, ProcessSet::SrcDst)
    }


    #[derive(Debug)]
    /// Блокировка одного процесса или пара блокировок двух разных процессов.
    pub(super) enum LockSet<'a> {
        /// Пара блокировок двух разных процессов.
        Different {
            /// Процесс, над которым совершается действие системного вызова.
            dst: SpinlockGuard<'a, Process>,

            /// Процесс, запустивший системный вызов.
            src: SpinlockGuard<'a, Process>,
        },

        /// Блокировка только процесса, над которым совершается действие системного вызова.
        /// Используется когда не нужна блокировка на процесс, запустивший системный вызов.
        Dst {
            /// Процесс, над которым совершается действие системного вызова.
            dst: SpinlockGuard<'a, Process>,
        },

        /// Блокировка одного процесса, который и запустил системный вызов
        /// и одновременно является целевым процессом для этого системного вызова.
        Same {
            /// Процесс, запустивший системный вызов на себя же.
            src_dst: SpinlockGuard<'a, Process>,
        },
    }


    impl<'a> LockSet<'a> {
        /// Проверяет, что процесс `src` имеет право модифицировать целевой процесс,
        /// заданный своим идентификатором `dst_pid`.
        /// Целевой процесс может совпадать с `src`.
        ///
        /// Модифицировать можно:
        ///   - Либо самого себя, задавая [`Pid::Current`] или явно собственный идентификатор [`Pid::Id`].
        ///   - Либо свой непосредственно дочерний процесс, задавая его идентификатор.
        ///
        /// Возвращает:
        ///   - Исходную блокировку на `src`, если `dst_pid` задаёт тот же процесс.
        ///   - Блокировку на процесс `dst`, если `process_set == ProcessSet::Dst`.
        ///   - Блокировки на процессы `src` и `dst`, если это разные процессы.
        ///
        /// Захватывает блокировки в правильном порядке для избежания
        /// [взаимоблокировки](https://en.wikipedia.org/wiki/Deadlock).
        fn new(
            src: SpinlockGuard<Process>,
            dst_pid: usize,
            process_set: ProcessSet,
        ) -> Result<LockSet<'_>> {
            // TODO: your code here.
            unimplemented!();
        }


        /// Возвращает процесс, над которым совершается действие системного вызова.
        #[allow(clippy::needless_arbitrary_self_type)]
        #[duplicate_item(
            dst_accessor reference(type);
            [dst] [&'b type];
            [dst_mut] [&'b mut type];
        )]
        pub(super) fn dst_accessor<'b>(
            self: reference([Self]),
        ) -> reference([SpinlockGuard<'a, Process>]) {
            match self {
                LockSet::Same { src_dst } => src_dst,
                LockSet::Different { dst, .. } => dst,
                LockSet::Dst { dst } => dst,
            }
        }


        /// Возвращает процесс, запустивший системный вызов.
        ///
        /// # Panics
        ///
        /// Паникует, если изначально блокировка захватывалась только на целевой процесс.
        #[allow(clippy::needless_arbitrary_self_type)]
        #[allow(dead_code)]
        #[duplicate_item(
            src_accessor reference(type);
            [src] [&'b type];
            [src_mut] [&'b mut type];
        )]
        pub(super) fn src_accessor<'b>(
            self: reference([Self]),
        ) -> reference([SpinlockGuard<'a, Process>]) {
            match self {
                LockSet::Same { src_dst } => src_dst,
                LockSet::Different { src, .. } => src,
                LockSet::Dst { .. } => panic!("only destination process is locked"),
            }
        }


        /// Возвращает `true`, если `src` и `dst` --- это один и тот же процесс.
        ///
        /// # Panics
        ///
        /// Паникует, если изначально блокировка захватывалась только на целевой процесс.
        #[allow(dead_code)]
        pub(super) fn is_same(&self) -> bool {
            match self {
                LockSet::Same { .. } => true,
                LockSet::Different { .. } => false,
                LockSet::Dst { .. } => panic!("only destination process is locked"),
            }
        }
    }


    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    /// Указывает какой набор процессов блокировать.
    enum ProcessSet {
        /// Блокировать только процесс, над которым совершается действие системного вызова.
        Dst,

        /// Блокировать оба процесса ---
        /// и процесс, запустивший системный вызов,
        /// и процесс, над которым совершается действие системного вызова.
        SrcDst,
    }
}


#[doc(hidden)]
pub mod test_scaffolding {
    use ku::{
        process::{MiniContext, SyscallResult},
        sync::spinlock::SpinlockGuard,
    };

    use crate::error::Result;

    use super::super::Process;


    pub fn log_value(
        process: SpinlockGuard<Process>,
        level: usize,
        start: usize,
        len: usize,
        value: usize,
    ) -> Result<SyscallResult> {
        super::log_value(process, level, start, len, value)
    }


    pub fn exofork(process: SpinlockGuard<Process>) -> Result<SyscallResult> {
        super::exofork(process, MiniContext::default())
    }


    pub fn map(
        process: SpinlockGuard<Process>,
        dst_pid: usize,
        dst_address: usize,
        dst_size: usize,
        flags: usize,
    ) -> Result<SyscallResult> {
        super::map(process, dst_pid, dst_address, dst_size, flags)
    }


    pub fn unmap(
        process: SpinlockGuard<Process>,
        dst_pid: usize,
        dst_address: usize,
        dst_size: usize,
    ) -> Result<SyscallResult> {
        super::unmap(process, dst_pid, dst_address, dst_size)
    }


    pub fn copy_mapping(
        process: SpinlockGuard<Process>,
        dst_pid: usize,
        src_address: usize,
        dst_address: usize,
        dst_size: usize,
        flags: usize,
    ) -> Result<SyscallResult> {
        super::copy_mapping(process, dst_pid, src_address, dst_address, dst_size, flags)
    }


    pub fn set_state(
        process: SpinlockGuard<Process>,
        dst_pid: usize,
        state: usize,
    ) -> Result<SyscallResult> {
        super::set_state(process, dst_pid, state)
    }
}
