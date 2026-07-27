use core::{alloc::Layout, cmp, mem::MaybeUninit, ptr::NonNull};

use alloc::alloc::alloc;

use crate::{
    error::Result,
    memory::{addr::IsVirt, mmu::PageTableFlags, size::SizeOf, Block, Page, Virt},
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


    /// Выделяет блок `block` виртуальной памяти.
    /// Ни выделения физической памяти, ни создания отображения станиц, не происходит.
    ///
    /// - Если выделить заданный блок виртуальной памяти не удалось,
    ///   например, он содержит уже выделенную страницу,
    ///   возвращает ошибку [`Error::NoPage`].
    fn reserve_fixed(&mut self, block: Block<Page>) -> Result<()>;


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
    ///
    /// Параметр `flags` задаёт флаги доступа к страницам `new_block`:
    ///   - [`None`] --- использовать те же флаги, что и в `old_block`,
    ///     индивидуально для каждой страницы.
    ///   - [`Some`] --- использовать для всех страниц флаги `flags`.
    ///
    /// В случае совпадения `old_block` и `new_block`, необходимо только задать флаги.
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
        let block = self.reserve(layout)?;

        let ptr = unsafe {
            self.map(block, self.flags())?;
            initialize_block(block, initialize)?;
            block.try_into_non_null_slice::<u8>()?
        };

        Ok(ptr)
    }


    unsafe fn dry_deallocate(&mut self, ptr: NonNull<u8>, layout: Layout) {
        let block = try_into_block(ptr, layout).unwrap();
        unsafe {
            self.unmap(block).unwrap();
            self.unreserve(block).unwrap();
        }
    }


    unsafe fn dry_grow(
        &mut self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
        initialize: Initialize,
    ) -> Result<NonNull<[u8]>> {
        let old_block = try_into_block(ptr, old_layout)?;
        let new_block = try_into_block(ptr, new_layout)?;

        if old_block == new_block {
            let new_ptr = unsafe { new_block.try_into_non_null_slice::<u8>()? };
            return Ok(new_ptr);
        }

        let new_ptr = self.dry_allocate(new_layout, initialize)?;
        let new_block = try_into_block(new_ptr.cast(), new_layout)?;

        let new_uncopied =
            Block::from_index(new_block.start(), new_block.start() + old_block.count())?;

        unsafe {
            self.copy_mapping(old_block, new_uncopied, None)?;
        }

        unsafe { self.dry_deallocate(ptr, old_layout) };

        Ok(new_ptr)
    }


    unsafe fn dry_shrink(
        &mut self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>> {
        let old_block = try_into_block(ptr, old_layout)?;
        let new_block = try_into_block(ptr, new_layout)?;

        if old_block == new_block {
            let new_ptr = unsafe { new_block.try_into_non_null_slice::<u8>()? };
            return Ok(new_ptr);
        }

        let empty_block = Block::from_index(new_block.end(), old_block.end())?;

        unsafe {
            self.unmap(empty_block)?;
            self.unreserve(empty_block)?;
        }

        let new_ptr = unsafe { new_block.try_into_non_null_slice::<u8>()? };
        Ok(new_ptr)
    }
}

#[inline]
const fn align_up(addr: usize, align: usize) -> usize {
    let align_mask = align - 1;

    if addr & align_mask == 0 {
        addr
    } else {
        if let Some(aligned) = (addr | align_mask).checked_add(1) {
            aligned
        } else {
            panic!("attempt to add with overflow")
        }
    }
}

/// Преобразует `ptr` и `layout` в соответствующий блок страниц.
///
/// Возвращает ошибки:
///   - [`Error::WrongAlignment`], если `ptr` не выровнен на границу страницы.
///   - [`Error::Overflow`] или [`Error::InvalidArgument`], если получающийся блок
///     пересекает границу одной из половин адресного пространства.
fn try_into_block(ptr: NonNull<u8>, layout: Layout) -> Result<Block<Page>> {
    let start_addr = ptr.as_ptr() as usize;
    let end_addr = align_up(start_addr + layout.size(), Page::SIZE);

    if start_addr % Page::SIZE != 0 || start_addr % layout.align() != 0 {
        return Err(Error::WrongAlignment);
    }

    let start = Page::new(Virt::from_ptr(start_addr as *mut u8))?;
    let end = Page::new(Virt::from_ptr(end_addr as *mut u8))?;

    Block::new(start, end)
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
    let slice = unsafe { block.try_into_mut_slice::<u8>()? };

    match initialize {
        Initialize::Garbage => {},
        Initialize::Zero => {
            slice.fill(0);
        },
    }

    Ok(())
}