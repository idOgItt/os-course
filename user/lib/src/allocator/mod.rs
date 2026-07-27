/// Аллокатор памяти общего назначения в пространстве пользователя, реализованный через
/// системные вызовы [`syscall::map()`], [`syscall::unmap()`] и [`syscall::copy_mapping()`].
mod map;


use core::alloc::Layout;

use ku::{
    allocator::{DetailedInfo, Dispatcher, Info},
    log::debug,
    sync::Spinlock,
};

use map::MapAllocator;

// Used in docs.
#[allow(unused)]
use crate::syscall;


/// Статистика глобального аллокатора памяти общего назначения в пространстве пользователя.
pub fn info() -> Info {
    GLOBAL_ALLOCATOR.info()
}


/// Распечатывает детальную статистику аллокатора.
pub fn dump_info() {
    /// Память под детальную статистику аллокатора.
    static DETAILED_INFO: Spinlock<DetailedInfo> = Spinlock::new(DetailedInfo::new());

    let mut allocator_info = DETAILED_INFO.lock();
    GLOBAL_ALLOCATOR.detailed_info(&mut allocator_info);
    debug!(%allocator_info);
}


/// Обработчик ошибок глобального аллокатора памяти общего назначения.
#[alloc_error_handler]
#[cold]
#[inline(never)]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("failed to allocate memory, layout = {:?}", layout)
}


/// Глобальный аллокатор памяти общего назначения в пространстве пользователя, реализованный через
/// системные вызовы [`syscall::map()`], [`syscall::unmap()`] и [`syscall::copy_mapping()`].
#[global_allocator]
static GLOBAL_ALLOCATOR: Dispatcher<MapAllocator> = Dispatcher::new(MapAllocator::new());
