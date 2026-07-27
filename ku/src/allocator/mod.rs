/// Интерфейс аллокатора памяти общего назначения [`BigAllocator`],
/// который умеет выдавать только выровненные на границу страниц блоки памяти.
pub mod big;

/// Определяет [`DryAllocator`], который аналогичен [`core::alloc::Allocator`],
/// и позволяет не дублировать код.
pub mod dry;

/// Статистика аллокатора общего назначения.
mod info;

/// Аллокатор верхнего уровня.
/// По запрошенному размеру определяет из какого аллокатора будет выделяться память.
mod dispatcher;

/// Аллокатор, выделяющий память блоками одинакового размера.
mod fixed_size;


pub use big::BigAllocator;
pub use dispatcher::{DetailedInfo, Dispatcher};
pub use dry::{DryAllocator, Initialize};
pub use info::{AtomicInfo, Info};
