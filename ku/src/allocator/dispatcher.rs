use core::{
    alloc::{Allocator, GlobalAlloc, Layout},
    cmp,
    fmt,
    ptr::{self, NonNull},
};

use crate::{
    memory::{Block, Page},
    sync::Spinlock,
};

use super::{
    dry::Initialize,
    fixed_size::{self, FixedSizeAllocator},
    info::{AtomicInfo, Info},
};


/// Аллокатор верхнего уровня.
/// По запрошенному размеру определяет из какого аллокатора будет выделяться память.
#[derive(Debug)]
pub struct Dispatcher<T: Allocator> {
    /// Аллокатор для выделения памяти постранично, в том числе остальным аллокаторам.
    fallback: T,

    /// Статистика аллокаций через [`Dispatcher::fallback`].
    fallback_info: AtomicInfo,

    /// Набор аллокаторов для разных размеров блоков.
    /// Каждый из них выделяет память блоками фиксированного размера.
    fixed_size: [Spinlock<FixedSizeAllocator>; FIXED_SIZE_COUNT],

    /// Общая статистика аллокатора.
    info: AtomicInfo,
}


impl<T: Allocator> Dispatcher<T> {
    /// Создаёт аллокатор верхнего уровня с заданным постраничным аллокатором `fallback`.
    pub const fn new(fallback: T) -> Self {
        Self {
            fallback,
            fallback_info: AtomicInfo::new(),
            fixed_size: [FIXED_SIZE_DUMMY; FIXED_SIZE_COUNT],
            info: AtomicInfo::new(),
        }
    }


    /// Общая статистика аллокатора.
    pub fn info(&self) -> Info {
        self.info.load()
    }


    /// Записывает в `detailed_info` статистику аллокатора.
    /// Как суммарную, так и с разбивкой по компонентам.
    ///
    /// В случае конкурентных запросов к аллокатору статистика может не быть консистентной.
    pub fn detailed_info(&self, detailed_info: &mut DetailedInfo) {
        detailed_info.total = self.info();
        detailed_info.fallback = self.fallback_info.load();

        for (info, allocator) in detailed_info.fixed_size.iter_mut().zip(&self.fixed_size) {
            *info = *allocator.lock().info();
        }
    }


    /// Находит индекс [`FixedSizeAllocator`], который отвечает за заданный `layout`.
    ///
    /// Возвращает [`None`], если такого нет.
    /// То есть, если за этот `layout` отвечает [`Dispatcher::fallback`].
    fn get_fixed_size_index(&self, layout: Layout) -> Option<usize> {
        // TODO: your code here.
        None // TODO: remove before flight.
    }


    /// Возвращает размер блоков памяти, за которые отвечает аллокатор
    /// по индексу `index` в массиве [`Dispatcher::fixed_size`].
    #[allow(unused)]
    fn get_size(index: usize) -> usize {
        get_size(index)
    }


    /// Выделяет блок памяти с помощью аллокатора `fallback`.
    fn fallback_allocate(&self, layout: Layout, initialize: Initialize) -> *mut u8 {
        let ptr = match initialize {
            Initialize::Garbage => self.fallback.allocate(layout),
            Initialize::Zero => self.fallback.allocate_zeroed(layout),
        };

        if let Ok(ptr) = ptr {
            self.update_fallback_info(Operation::Allocation, layout);

            ptr.as_non_null_ptr().as_ptr()
        } else {
            ptr::null_mut()
        }
    }


    /// Выделяет блок памяти с помощью аллокатора
    /// по индексу `index` в массиве [`Dispatcher::fixed_size`].
    fn fixed_size_allocate(&self, index: usize, layout: Layout) -> *mut u8 {
        // TODO: your code here.
        unimplemented!();
    }


    /// Проверяет, что `layout` поддерживается.
    ///
    /// # Panics
    ///
    /// Паникует, если `layout` не поддерживается.
    fn validate_layout(layout: Layout) {
        if layout.size() == 0 {
            panic!("can not handle zero-sized types requested by {:?}", layout);
        }

        if layout.align() > Page::SIZE {
            panic!(
                "can not handle alignment of {:?} that is greater than the page size {}",
                layout,
                Page::SIZE,
            );
        }
    }


    /// Обновляет статистики [`Dispatcher::info`] и [`Dispatcher::fallback_info`],
    /// записывая в них операцию `operation` с заданным `layout`,
    /// которая была обслужена аллокатором [`Dispatcher::fallback`].
    fn update_fallback_info(&self, operation: Operation, layout: Layout) {
        let allocated_pages = layout.size().div_ceil(Page::SIZE);
        let allocated = allocated_pages * Page::SIZE;

        for info in [&self.fallback_info, &self.info] {
            match operation {
                Operation::Allocation => {
                    info.allocation(layout.size(), allocated, allocated_pages);
                },
                Operation::Deallocation => {
                    info.deallocation(layout.size(), allocated, allocated_pages);
                },
            }
        }
    }
}


unsafe impl<T: Allocator> GlobalAlloc for Dispatcher<T> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        Self::validate_layout(layout);

        // TODO: your code here.

        self.fallback_allocate(layout, Initialize::Garbage)
    }


    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        Self::validate_layout(layout);

        // TODO: your code here.

        let ptr = NonNull::new(ptr).expect("should not get null ptr in dealloc()");

        unsafe {
            self.fallback.deallocate(ptr, layout);
        }

        self.update_fallback_info(Operation::Deallocation, layout);
    }


    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        Self::validate_layout(layout);

        // TODO: your code here.

        self.fallback_allocate(layout, Initialize::Zero)
    }


    unsafe fn realloc(&self, old_ptr: *mut u8, old_layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = Layout::from_size_align(new_size, old_layout.align())
            .expect("bad `old_layout` or `new_size` for realloc");

        Self::validate_layout(new_layout);

        // TODO: your code here.

        let old_ptr = NonNull::new(old_ptr).expect("should not get null ptr in realloc()");

        let new_ptr = if old_layout.size() < new_layout.size() {
            unsafe { self.fallback.grow(old_ptr, old_layout, new_layout) }
        } else {
            unsafe { self.fallback.shrink(old_ptr, old_layout, new_layout) }
        };

        if let Ok(new_ptr) = new_ptr {
            self.update_fallback_info(Operation::Allocation, new_layout);
            self.update_fallback_info(Operation::Deallocation, old_layout);

            new_ptr.as_non_null_ptr().as_ptr()
        } else {
            ptr::null_mut()
        }
    }
}


/// Детальная статистика аллокатора [`Dispatcher`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetailedInfo {
    /// Общая статистика аллокатора [`Dispatcher`].
    total: Info,

    /// Статистика аллокаций через [`Dispatcher::fallback`].
    fallback: Info,

    /// Статистика аллокаторов [`FixedSizeAllocator`] для разных размеров блоков.
    fixed_size: [Info; FIXED_SIZE_COUNT],
}


#[allow(rustdoc::private_intra_doc_links)]
impl DetailedInfo {
    /// Инициализирует детальную статистику аллокатора [`Dispatcher`] нулями.
    pub const fn new() -> Self {
        Self {
            total: Info::new(),
            fallback: Info::new(),
            fixed_size: [Info::new(); FIXED_SIZE_COUNT],
        }
    }


    /// Общая статистика аллокатора [`Dispatcher`].
    pub fn total(&self) -> &Info {
        &self.total
    }


    /// Статистика аллокаций через [`Dispatcher::fallback`].
    pub fn fallback(&self) -> &Info {
        &self.fallback
    }


    /// Статистика аллокаторов [`FixedSizeAllocator`] для разных размеров блоков.
    pub fn fixed_size(&self) -> &[Info] {
        &self.fixed_size
    }


    /// Проверяет инварианты статистики аллокатора.
    ///
    /// Требует эксклюзивного доступа к аллокатору в момент снятия детальной статистики.
    /// Иначе из-за конкурентных операций статистики разойдутся.
    pub fn is_valid(&self) -> bool {
        if !self.total.is_valid() || !self.fallback.is_valid() {
            return false;
        }

        if let Ok(mut balance) = self.total - self.fallback {
            // `.iter()` is required to avoid copying `self.fixed_size`
            // which is too large to fit on the stack.
            // See also:
            //   - <https://github.com/rust-lang/rust/issues/45683>
            //   - <https://crates.io/crates/cargo-call-stack>
            for fixed_size in self.fixed_size.iter() {
                if !fixed_size.is_valid() {
                    return false;
                }

                if let Ok(new_balance) = balance - *fixed_size {
                    balance = new_balance;
                } else {
                    return false;
                }
            }

            balance == Info::new()
        } else {
            false
        }
    }
}


impl Default for DetailedInfo {
    fn default() -> Self {
        Self::new()
    }
}


// Avoid changing the info during formatting it due to requests to the global allocator.
impl fmt::Display for DetailedInfo {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let separator = if formatter.alternate() { "\n    " } else { " " };

        write!(
            formatter,
            "{{{0}valid: {1},{0}total: {2},{0}fallback: {3}",
            separator,
            self.is_valid(),
            self.total,
            self.fallback,
        )?;

        for (index, fixed_size) in self.fixed_size.iter().enumerate() {
            if fixed_size.allocations().positive() > 0 {
                let size = get_size(index);
                write!(formatter, ",{}size_{}: {}", separator, size, fixed_size)?;
            }
        }

        if formatter.alternate() && self.total.allocated().positive() > 0 {
            assert!(self.total.allocations().positive() > 0);
            assert!(self.total.pages().positive() > 0);

            let total_allocations = self.total.allocations().positive() as f64;
            let total_allocated = self.total.allocated().positive() as f64;
            let total_pages = self.total.pages().positive() as f64;

            let format_ratio = |formatter: &mut fmt::Formatter, info: &Info| {
                write!(
                    formatter,
                    "{{ allocations: {:.3}%, allocated: {:.3}%, pages: {:.3}% }}",
                    info.allocations().positive() as f64 / total_allocations * 100.0,
                    info.allocated().positive() as f64 / total_allocated * 100.0,
                    info.pages().positive() as f64 / total_pages * 100.0,
                )
            };

            write!(formatter, ",{0}allotment: {{{0}    fallback: ", separator)?;
            format_ratio(formatter, &self.fallback)?;

            for (index, fixed_size) in self.fixed_size.iter().enumerate() {
                if fixed_size.allocations().positive() > 0 {
                    let size = get_size(index);
                    write!(formatter, ",{}    size_{}: ", separator, size)?;
                    format_ratio(formatter, fixed_size)?;
                }
            }

            write!(formatter, ",{}}}", separator)?;
        }

        let separator = if formatter.alternate() { ",\n" } else { " " };

        write!(formatter, "{}}}", separator)
    }
}


/// Операция, которая была выполнена.
#[derive(Debug)]
enum Operation {
    /// Выделение памяти.
    Allocation,

    /// Освобождение памяти.
    Deallocation,
}


/// Возвращает размер блоков памяти, за которые отвечает аллокатор
/// по индексу `index` в массиве [`Dispatcher::fixed_size`].
fn get_size(index: usize) -> usize {
    (index + 1) * fixed_size::MIN_SIZE
}


/// Количество аллокаторов фиксированного размера [`FixedSizeAllocator`],
/// которыми управляет [`Dispatcher`].
const FIXED_SIZE_COUNT: usize = (4 * Page::SIZE - 1) / fixed_size::MIN_SIZE;

/// Вспомогательная константа для инициализации массива [`Dispatcher::fixed_size`]
/// в константной функции [`Dispatcher::new()`].
/// Необходима, так как [`Spinlock`] не может быть помечен типажём [`Copy`].
#[allow(clippy::declare_interior_mutable_const)]
const FIXED_SIZE_DUMMY: Spinlock<FixedSizeAllocator> = Spinlock::new(FixedSizeAllocator::new());
