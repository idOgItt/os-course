use core::{alloc::Layout, mem};

use crate::{
    error::Error::Null,
    memory::{Block, Page, Virt},
};

use super::Info;


/// Аллокатор, выделяющий память блоками одинакового размера.
///
/// Предполагает, что все блоки памяти, с которыми он работает,
/// имеют одинаковый размер.
/// Причём тот же размер передаётся как аргумент `size` в
/// некоторые из его методов.
/// Более того, этот размер должен быть кратен [`MIN_SIZE`].
#[derive(Debug)]
pub(super) struct FixedSizeAllocator {
    /// Список свободных блоков памяти.
    free: Lifo,

    /// Статистика аллокатора.
    info: Info,
}


impl FixedSizeAllocator {
    /// Создаёт аллокатор, выделяющий память блоками одинакового размера.
    pub(super) const fn new() -> FixedSizeAllocator {
        FixedSizeAllocator {
            free: Lifo::new(),
            info: Info::new(),
        }
    }


    /// Статистика аллокатора.
    pub(super) fn info(&self) -> &Info {
        &self.info
    }


    /// Статистика аллокатора.
    pub(super) fn info_mut(&mut self) -> &mut Info {
        &mut self.info
    }


    /// Возвращает `true`, если у аллокатора больше нет свободных блоков.
    pub(super) fn is_empty(&self) -> bool {
        self.free.is_empty()
    }


    /// Возвращает [`Layout`] блока памяти,
    /// который состоит из целого количества страниц [`Page`].
    ///
    /// В методе [`FixedSizeAllocator::stock_up()`] блок с этим `Layout`
    /// будет нарезаться на подблоки размера `size`, которые выделяет этот аллокатор.
    pub(super) fn get_pack_layout(size: usize) -> Layout {
        // TODO: your code here.
        unimplemented!();
    }


    /// Режет блок `pack` на подблоки размера `size`,
    /// которые сохраняет себе для последующих аллокаций.
    ///
    /// # Panics
    ///
    /// Паникует, если:
    ///   - `pack` не выровнен по границам страниц;
    ///   - `size` не подходит под требования размера и выравнивания структуры [`Lifo`].
    pub(super) fn stock_up(&mut self, size: usize, pack: Block<Virt>) {
        // TODO: your code here.
        unimplemented!();
    }


    /// Выделяет блок памяти из списка свободных.
    /// Возвращает указатель на выделенный блок или
    /// [`core::ptr::null_mut()`], если список свободных блоков пуст.
    pub(super) fn allocate(&mut self, layout: Layout, size: usize) -> *mut u8 {
        // TODO: your code here.
        unimplemented!();
    }


    /// Сохраняет в список свободных блок памяти по указателю `ptr`.
    ///
    /// # Safety
    ///
    /// Вызывающая сторона должна гарантировать
    /// [то же самое](https://doc.rust-lang.org/nightly/core/alloc/trait.Allocator.html#safety-1),
    /// что требуется при вызове [`core::alloc::Allocator::deallocate()`].
    pub(super) unsafe fn deallocate(&mut self, ptr: *mut u8, layout: Layout, size: usize) {
        // TODO: your code here.
        unimplemented!();
    }
}


/// Список свободных блоков памяти.
///
/// Узел списка является этой же структурой.
/// Фактически, узел задаёт подсписок от текущего узла до последнего.
///
/// В названии используется аббревиатура LIFO
/// ([Last In, First Out](https://en.wikipedia.org/wiki/Stack_(abstract_data_type))),
/// а не `Stack`, чтобы не путать со стеком программы.
#[derive(Clone, Copy, Debug)]
struct Lifo {
    /// Адрес следующего свободного блока.
    next: Virt,
}


impl Lifo {
    /// Создаёт пустой список свободных блоков памяти.
    const fn new() -> Self {
        Self { next: Virt::zero() }
    }


    /// Возвращает `true`, если в списке больше нет свободных блоков.
    fn is_empty(&self) -> bool {
        self.next == Virt::zero()
    }


    /// Выделяет блок памяти из списка свободных.
    /// Возвращает адрес выделенного блока или
    /// [`Virt::zero()`], если список свободных блоков пуст.
    fn pop(&mut self) -> Virt {
        // TODO: your code here.
        unimplemented!();
    }


    /// Сохраняет в список свободных блок памяти с адресом `virt`.
    ///
    /// # Panics
    ///
    /// Паникует, если `virt` не является валидным адресом для хранения структуры [`Lifo`].
    fn push(&mut self, virt: Virt) {
        // TODO: your code here.
        unimplemented!();
    }
}


/// Минимальный размер и выравнивание для блоков памяти, которыми управляет [`FixedSizeAllocator`].
pub(super) const MIN_SIZE: usize = if mem::size_of::<Lifo>() > mem::align_of::<Lifo>() {
    mem::size_of::<Lifo>()
} else {
    mem::align_of::<Lifo>()
};
