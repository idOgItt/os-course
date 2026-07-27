/// Отладочная информация [`Callsite`] о точке вызова некоторой функции.
pub mod callsite;


use core::{arch::asm, fmt, mem};

use derive_more::Display;

use crate::{
    error::Result,
    memory::{
        addr::{Tag, VirtTag},
        Block,
        Virt,
    },
};

pub use callsite::Callsite;


/// Поддержка печати бектрейсов.
///
/// Бектрейс можно расшифровать командой `llvm-symbolizer`, например
/// ```console
/// ~/nikka$ cd kernel
/// ~/nikka/kernel$ cargo test --test 5-um-3-eager-fork
/// ...
/// 23:02:48.579 0 E panicked at 'I did my best, it wasn't much', user/eager_fork/src/main.rs:47:5; backtrace = 0x10008593 0x10008479 0x100085B8 0x10008CD9; pid = 2:1
/// ~/nikka/kernel$ echo '0x10008593 0x10008479 0x100085B8 0x10008CD9' | tr ' ' '\n' | llvm-symbolizer --exe ../target/kernel/user/eager_fork
/// eager_fork::fork_tree::h6af1cf920637510d
/// .../nikka/user/eager_fork/src/main.rs:47:5
///
/// eager_fork::main::h7eeb3104124da585
/// .../nikka/user/eager_fork/src/main.rs:40:1
///
/// main
/// .../nikka/user/lib/src/lib.rs:88:10
///
/// _start
/// .../nikka/user/lib/src/lib.rs:60:19
/// ```
///
/// Требует от компилятора
///   - генерации указателей фреймов (force-frame-pointers=yes) и
///   - расположения кода по фиксированным адресам (relocation-model=dynamic-no-pic).
///
/// Для этого нужно добавить в `.cargo/config.toml` опции
/// ```toml
/// [build]
///     rustflags = [
///         "--codegen", "force-frame-pointers=yes",
///         "--codegen", "relocation-model=dynamic-no-pic",
///     ]
/// ```
#[derive(Clone, Copy, Default)]
pub struct Backtrace {
    /// Адрес, ниже которого не может быть расположен внешний фрейм, --- стек растёт вниз.
    /// Снижает вероятность некорректного обращения к памяти
    /// при поиске конца списка стековых фреймов.
    lower_limit: Virt,

    /// Стек, знание которого позволяет найти конец списка стековых фреймов.
    /// Радикально снижает вероятность некорректного обращения к памяти
    /// при поиске конца списка стековых фреймов.
    /// Если не известен, устанавливается равным одной странице памяти,
    /// в которую указывает регистр `RBP`.
    stack: Block<Virt>,

    /// Текущий стековый фрейм.
    stack_frame: StackFrame,
}


impl Backtrace {
    /// Возвращает бектрейс по значениям регистров `rbp` и `rsp`.
    ///
    /// Эта функция инлайнится,
    /// чтобы не порождать дополнительный стековый фрейм под её вызов и не замусоривать бектрейс.
    /// В результате, функция вызвавшая [`Backtrace::current()`], в него не попадёт.
    #[inline(always)]
    pub fn new(rbp: usize, rsp: usize) -> Result<Self> {
        let lower_limit = Virt::new(rsp)?;
        let stack = Block::from_index(rbp, rbp + 1)?.enclosing().into();

        Ok(Self {
            lower_limit,
            stack,
            stack_frame: StackFrame::new(rbp)?,
        })
    }


    /// Возвращает текущий бектрейс.
    ///
    /// Эта функция инлайнится,
    /// чтобы не порождать дополнительный стековый фрейм под её вызов и не замусоривать бектрейс.
    /// В результате, функция вызвавшая [`Backtrace::current()`], в него не попадёт.
    #[inline(always)]
    pub fn current() -> Result<Self> {
        let rsp: usize;
        unsafe {
            asm!(
                "mov {0}, rsp",
                out(reg) rsp,
            );
        }

        Self::new(rbp(), rsp)
    }


    /// Возвращает текущий бектрейс.
    ///
    /// `stack` --- текущий стек, знание которого позволяет найти конец списка стековых фреймов.
    ///
    /// Эта функция инлайнится,
    /// чтобы не порождать дополнительный стековый фрейм под её вызов и не замусоривать бектрейс.
    /// В результате, функция вызвавшая [`Backtrace::with_stack()`], в него не попадёт.
    #[inline(always)]
    pub fn with_stack(stack: Block<Virt>) -> Result<Self> {
        let mut backtrace = Self::current()?;
        backtrace.stack = stack;
        Ok(backtrace)
    }
}


impl Iterator for Backtrace {
    type Item = StackFrame;


    fn next(&mut self) -> Option<Self::Item> {
        if let Some(outer) = self.stack_frame.outer(&mut self.lower_limit, self.stack) {
            self.stack_frame = outer;
            Some(outer)
        } else {
            None
        }
    }
}


impl fmt::Debug for Backtrace {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "Backtrace:")?;

        for stack_frame in *self {
            write!(formatter, "\n  {}", stack_frame)?;
        }

        Ok(())
    }
}


impl fmt::Display for Backtrace {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let mut separator = "";

        for stack_frame in *self {
            write!(formatter, "{}{}", separator, stack_frame)?;
            separator = " ";
        }

        Ok(())
    }
}


/// Узел списка стековых фреймов.
#[derive(Clone, Copy, Debug, Default, Display)]
#[display("{:#X}", return_address)]
#[repr(C)]
pub struct StackFrame {
    /// Адрес внешнего стекового фрейма --- фрейма вызвавшей функции.
    outer: usize,

    /// Адрес возврата в вызвавшую функцию.
    return_address: usize,
}


impl StackFrame {
    /// Возвращает стековый фрейм по значению регистра `rbp`.
    ///
    /// Эта функция инлайнится,
    /// чтобы не порождать дополнительный стековый фрейм под её вызов и не замусоривать бектрейс.
    #[inline(always)]
    pub fn new(rbp: usize) -> Result<Self> {
        Self::validate(rbp)
    }


    /// Возвращает текущий стековый фрейм.
    ///
    /// Эта функция инлайнится,
    /// чтобы не порождать дополнительный стековый фрейм под её вызов и не замусоривать бектрейс.
    #[inline(always)]
    pub fn current() -> Result<Self> {
        Self::new(rbp())
    }


    /// Адрес возврата в вызвавшую функцию.
    pub fn return_address(&self) -> Virt {
        Virt::new(self.return_address).expect("incorrect stack frame")
    }


    /// Возвращает внешний стековый фрейма или `None`, если текущий фрейм самый внешний.
    fn outer(&self, lower_limit: &mut Virt, stack: Block<Virt>) -> Option<Self> {
        let outer_start = Virt::new(self.outer).expect("incorrect stack frame");

        if outer_start == Virt::default() ||
            !VirtTag::is_same_area(outer_start, *lower_limit) ||
            outer_start < *lower_limit
        {
            return None;
        }

        let outer_end = (outer_start + mem::size_of::<StackFrame>()).ok()?;
        let outer = Block::new(outer_start, outer_end).ok()?;

        if let Ok(new_lower_limit) = outer.start_address() + mem::size_of::<StackFrame>() {
            *lower_limit = new_lower_limit;
        }

        if stack.contains_block(outer) {
            Self::validate(self.outer).ok()
        } else {
            None
        }
    }


    /// Проверяет, что `stack_frame` является валидным адресом стекового фрейма и
    /// по нему расположен валидный стековый фрейм.
    /// В случае успеха возвращает копию этого стекового фрейма.
    ///
    /// # Note
    ///
    /// В случае успешного результата нет абсолютной гарантии,
    /// что по адресу `stack_frame` расположен именно стековый фрейм.
    /// Так как выполняемые проверки могут пройти успешно случайно,
    /// если по адресу `stack_frame` находится не стековый фрейм,
    /// а удовлетворяющий критериям проверок мусор.
    fn validate(stack_frame: usize) -> Result<Self> {
        let stack_frame = unsafe { *Virt::new(stack_frame)?.try_into_ref::<Self>()? };

        Virt::new(stack_frame.outer)?;
        Virt::new(stack_frame.return_address)?;

        Ok(stack_frame)
    }
}


/// Возвращает содержимое регистра `RBP`.
///
/// Эта функция инлайнится,
/// чтобы не порождать дополнительный стековый фрейм под её вызов и не замусоривать бектрейс.
#[inline(always)]
fn rbp() -> usize {
    let rbp: usize;
    unsafe {
        asm!(
            "mov {0}, rbp",
            out(reg) rbp,
        );
    }

    rbp
}
