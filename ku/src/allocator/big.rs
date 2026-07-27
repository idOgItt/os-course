use core::{alloc::Layout, cmp, ptr::NonNull};

use crate::{
    error::Result,
    memory::{mmu::PageTableFlags, size::SizeOf, Block, Page, Virt},
};

use super::{DryAllocator, Initialize};

// Used in docs.
#[allow(unused)]
use {crate::error::Error, core::alloc::Allocator};


/// Интерфейс аллокатора памяти общего назначения,
/// который умеет выдавать только выровненные на границу страниц блоки памяти.
///
/// # Safety
///
/// Код, который реализует этот типаж должен гарантировать,
/// что инварианты управления памятью в Rust'е не будут нарушены.
/// В частности, [`BigAllocator::reserve()`] не должен выдавать
/// занятые в данный момент блоки виртуальных адресов.
pub unsafe trait BigAllocator {
    /// Возвращает текущие флаги отображения страниц.
    ///
    /// Они используются для аллокаций из [`BigAllocator`] через интерфейсы
    /// [`core::alloc::Allocator`] или [`DryAllocator`],
    /// которые не принимают на вход флагов отображений страниц.
    fn flags(&self) -> PageTableFlags;


    /// Устанавливает текущие флаги отображения страниц.
    ///
    /// Они используются для аллокаций из [`BigAllocator`] через интерфейсы
    /// [`core::alloc::Allocator`] или [`DryAllocator`],
    /// которые не принимают на вход флагов отображений страниц.
    ///
    /// Возвращает ошибку:
    ///   - [`Error::PermissionDenied`] если запрошенные флаги
    ///     не допускаются этим аллокатором.
    fn set_flags(&mut self, flags: PageTableFlags) -> Result<()>;


    /// Выделяет новый блок подряд идущих виртуальных страниц.
    /// Достаточный для хранения объекта, размер и выравнивание которого описывается `layout`.
    /// Ни выделения физической памяти, ни создания отображения станиц, не происходит.
    ///
    /// - Если выделить заданный размер виртуальной памяти не удалось,
    ///   возвращает ошибку [`Error::NoPage`].
    /// - Если заданный `layout` имеет выравнивание больше размера страницы,
    ///   возвращает ошибку [`Error::WrongAlignment`].
    fn reserve(&mut self, layout: Layout) -> Result<Block<Page>>;


    /// Освобождает блок виртуальных страниц `block`.
    ///
    /// # Safety
    ///
    /// - `block` должен был быть ранее выделен с помощью [`BigAllocator::reserve()`].
    /// - Отображения этих станиц в физическую память уже не дожно быть.
    /// - Вызывающий код должен гарантировать,
    ///   что инварианты управления памятью в Rust'е не будут нарушены.
    ///   В частности, не осталось ссылок, которые ведут в `block`.
    unsafe fn unreserve(&mut self, block: Block<Page>) -> Result<()>;


    /// Уменьшает ранее зарезервированный блок виртуальных страниц `old_block`
    /// до его подблока `sub_block`.
    /// Возвращает ошибку [`Error::InvalidArgument`], если `sub_block`
    /// не содержится в `old_block` целиком.
    ///
    /// # Safety
    ///
    /// - `old_block` должен был быть ранее выделен с помощью [`BigAllocator::reserve()`].
    /// - Для станиц `old_block`, не попадающих в `sub_block`,
    ///   отображения в физическую память уже не дожно быть.
    /// - Вызывающий код должен гарантировать,
    ///   что инварианты управления памятью в Rust'е не будут нарушены.
    ///   В частности, не осталось ссылок, которые ведут в освобождаемые страницы.
    unsafe fn rereserve(&mut self, old_block: Block<Page>, sub_block: Block<Page>) -> Result<()>;


    /// Аллоцирует нужное количество физических фреймов
    /// и отображает в них заданный блок виртуальных страниц `block` с флагами `flags`.
    ///
    /// Возвращает ошибку:
    ///   - [`Error::PermissionDenied`] если запрошенные флаги
    ///     не допускаются этим аллокатором.
    ///
    /// # Safety
    ///
    /// Вызывающий код должен гарантировать, что инварианты управления памятью в Rust'е
    /// не будут нарушены.
    /// В частности, не осталось ссылок, которые ведут в `block`.
    unsafe fn map(&mut self, block: Block<Page>, flags: PageTableFlags) -> Result<()>;


    /// Удаляет отображение заданного блока виртуальных страниц `block`.
    /// Физические фреймы, на которые не осталось других ссылок, освобождаются.
    /// После работы [`BigAllocator::unmap()`] виртуальные адреса `block`
    /// становятся недоступны.
    ///
    /// # Safety
    ///
    /// Вызывающий код должен гарантировать, что инварианты управления памятью в Rust'е
    /// не будут нарушены.
    /// В частности, не осталось ссылок, которые ведут в `block`.
    unsafe fn unmap(&mut self, block: Block<Page>) -> Result<()>;


    /// Копирует отображение физических фреймов из `old_block` в `new_block`.
    /// Если изначально `new_block` содержал отображённые страницы,
    /// их отображение удаляется.
    /// Физические фреймы, на которые не осталось других ссылок, освобождаются.
    /// А содержимое памяти, которое ранее было доступно через `old_block`,
    /// становится доступным и через `new_block`.
    /// Параметр `flags` задаёт флаги доступа к страницам `new_block`:
    ///   - [`None`] --- использовать те же флаги, что и в `old_block`,
    ///     индивидуально для каждой страницы.
    ///   - [`Some`] --- использовать для всех страниц флаги `flags`.
    ///
    /// Возвращает ошибки:
    ///   - [`Error::InvalidArgument`] если `old_block` и `new_block` имеют разный размер.
    ///   - [`Error::InvalidArgument`] если `old_block` и `new_block` пересекаются, но
    ///     не совпадают, или же `flags == None`.
    ///     (Ситуация смены флагов на одном и том же блоке является единственной допустимой
    ///     при пересечении `old_block` и `new_block`).
    ///   - [`Error::PermissionDenied`] если запрошенные флаги
    ///     не допускаются этим аллокатором.
    ///
    /// # Safety
    ///
    /// Вызывающий код должен гарантировать, что инварианты управления памятью в Rust'е
    /// не будут нарушены.
    /// В частности, не возникнет неуникальных мутабельных ссылок.
    unsafe fn copy_mapping(
        &mut self,
        old_block: Block<Page>,
        new_block: Block<Page>,
        flags: Option<PageTableFlags>,
    ) -> Result<()>;
}


unsafe impl<T: BigAllocator> DryAllocator for T {
    fn dry_allocate(&mut self, layout: Layout, initialize: Initialize) -> Result<NonNull<[u8]>> {
        // TODO: your code here.
        unimplemented!();
    }


    unsafe fn dry_deallocate(&mut self, ptr: NonNull<u8>, layout: Layout) {
        // TODO: your code here.
        unimplemented!();
    }


    unsafe fn dry_grow(
        &mut self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
        initialize: Initialize,
    ) -> Result<NonNull<[u8]>> {
        // TODO: your code here.
        unimplemented!();
    }


    unsafe fn dry_shrink(
        &mut self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>> {
        // TODO: your code here.
        unimplemented!();
    }
}


/// Преобразует `ptr` и `layout` в соответствующий блок страниц.
///
/// Возвращает ошибки:
///   - [`Error::WrongAlignment`], если `ptr` не выровнен на границу страницы.
///   - [`Error::Overflow`] или [`Error::InvalidArgument`], если получающийся блок
///     пересекает границу одной из половин адресного пространства.
fn try_into_block(ptr: NonNull<u8>, layout: Layout) -> Result<Block<Page>> {
    // TODO: your code here.
    unimplemented!();
}


/// Инициализирует `block` так, как предписывает `initialize`.
///
/// # Safety
///
/// - `block` должен быть отображён в память.
/// - Вызывающий код должен гарантировать,
///   что инварианты управления памятью в Rust'е не будут нарушены.
///   В частности, нет ссылок, которые ведут в `block`.
unsafe fn initialize_block(block: Block<Page>, initialize: Initialize) -> Result<()> {
    // TODO: your code here.
    unimplemented!();
}
