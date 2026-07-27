use core::{
    any,
    fmt,
    mem::{self, MaybeUninit},
    ptr,
};

use itertools::Itertools;
// use x86_64::structures::idt::ExceptionVector::Page;
use ku::sync::spinlock::Spinlock;

use crate::{
    allocator::Big,
    error::{
        Error::{InvalidArgument, NoPage, PermissionDenied, WrongAlignment},
        Result,
    },
    log::{debug, info, trace, warn},
    process::Pid,
};

use super::{block::Block, frage::{Frame, Page}, is_user_page, mapping::Mapping, mmu::{self, PageTableFlags}, page_allocator::PageAllocator, Virt, FRAME_ALLOCATOR, KERNEL_PAGE_ALLOCATOR};

// Used in docs.
#[allow(unused)]
use crate::{self as kernel, error::Error};
use crate::memory::mapping::Path;

/// Структура для работы с виртуальным адресным пространством.
#[derive(Debug, Default)]
pub struct AddressSpace {
    /// Тип виртуального адресного пространства для логирования:
    ///   - [`Kind::Invalid`] --- некорректное;
    ///   - [`Kind::Base`] --- базовое;
    ///   - [`Kind::Process`] --- относящееся к некоторому процессу.
    kind: Kind,

    /// Структура для управления отображением виртуальных страниц в физические фреймы.
    mapping: Mapping,

    /// Аллокатор виртуальных страниц.
    user_page_allocator: PageAllocator,

    /// Процесс, к которому относится данное адресное пространство.
    /// Содержит [`None`], если:
    ///   - у процесса ещё нет [`Pid`] или
    ///   - это адресное пространство не относится ни к какому процессу.
    pid: Option<Pid>,
}


impl AddressSpace {
    /// Инициализирует виртуальное адресное пространство текущим страничным отображением.
    pub(super) fn new(page_table_root: Frame, phys2virt: Page) -> Self {
        let mapping = Mapping::new(page_table_root, phys2virt);
        let user_page_allocator = PageAllocator::new(
            unsafe { mapping.page_table_ref(mapping.page_table_root()) },
            super::user_root_level_entries(),
        );

        let address_space = Self {
            kind: Kind::Base,
            mapping,
            user_page_allocator,
            pid: None,
        };

        info!(%address_space, "init");

        address_space
    }


    /// Возвращает пустое [`AddressSpace`].
    /// В отличие от [`AddressSpace::default()`] доступна в константном контексте.
    pub(crate) const fn zero() -> Self {
        Self {
            kind: Kind::Base,
            mapping: Mapping::new(Frame::zero(), Page::zero()),
            user_page_allocator: PageAllocator::zero(),
            pid: None,
        }
    }


    /// Создаёт копию адресного пространства с копией страничного отображения.
    /// См. [`Mapping::duplicate()`].
    pub(crate) fn duplicate(&self) -> Result<Self> {
        let address_space = Self {
            kind: Kind::Process,
            mapping: self.mapping.duplicate()?,
            user_page_allocator: self.user_page_allocator.duplicate(),
            pid: None,
        };

        info!(%address_space, "duplicate");

        Ok(address_space)
    }


    /// Устанавливает [`AddressSpace::user_page_allocator`] в состояние эквивалентное `original`.
    /// Текущий [`AddressSpace`] должен был быть получен
    /// из `original` методом [`AddressSpace::duplicate()`].
    /// И из него не должно было быть выделено больше страниц, чем из `original`.
    /// Если это не так, возвращается ошибка [`Error::InvalidArgument`].
    pub(crate) fn duplicate_allocator_state(&mut self, original: &AddressSpace) -> Result<()> {
        self.user_page_allocator
            .duplicate_allocator_state(&original.user_page_allocator)
    }


    /// Постраничный аллокатор памяти в этом адресном пространстве.
    /// Выделяемая им память будет отображена с флагами `flags`.
    pub fn allocator(&mut self, flags: PageTableFlags) -> Big {
        Big::new(self, flags)
    }


    /// Возвращает страничное отображение данного виртуального адресного пространства.
    pub fn mapping(&mut self) -> &mut Mapping {
        &mut self.mapping
    }


    /// Возвращает физический фрейм, в котором хранится корневой узел
    /// страничного отображения данного виртуального адресного пространства.
    pub fn page_table_root(&self) -> Frame {
        self.mapping.page_table_root()
    }


    /// Сохраняет информацию об идентификаторе процесса,
    /// который владеет этим адресным пространством.
    pub(crate) fn set_pid(&mut self, pid: Pid) {
        self.pid = Some(pid)
    }


    /// Переключает процессор в это виртуальное адресное пространство.
    pub(crate) fn switch_to(&self) {
        info!(address_space = %self, "switch to");

        unsafe {
            assert!(self.page_table_root() != Frame::default());
            mmu::set_page_table_root(self.page_table_root());
        }
    }


    /// Выделяет блок подряд идущих виртуальных страниц, достаточный для хранения объекта
    /// размером `size` **байт**.
    /// Ни выделения физической памяти, ни создания отображения станиц, не происходит.
    ///
    /// Если выделить заданный размер виртуальной памяти не удалось,
    /// возвращает ошибку [`Error::NoPage`].
    pub fn allocate(&mut self, size: usize, flags: PageTableFlags) -> Result<Block<Page>> {
        if flags.contains(PageTableFlags::USER_ACCESSIBLE) {
            self.user_page_allocator.allocate(size)
        } else {
            KERNEL_PAGE_ALLOCATOR.lock().allocate(size)
        }
    }


    /// Обратный метод к [`AddressSpace::allocate()`].
    /// Освобождает блок виртуальных страниц `block`.
    /// Ни освобождения физической памяти, ни модификации отображения станиц, не происходит.
    ///
    /// Возвращает ошибки:
    ///   - [`Error::InvalidArgument`] если заданный блок не является целиком аллоцированным.
    ///   - [`Error::PermissionDenied`] если в блоке есть как страницы,
    ///     которые лежит в зарезервированной для пользователя области,
    ///     так и страницы, которые лежат в зарезервированной для ядра области.
    pub fn deallocate(&mut self, block: Block<Page>) -> Result<()> {
        if super::is_user_block(block) {
            self.user_page_allocator.deallocate(block)
        } else if super::user_pages().is_disjoint(block) {
            KERNEL_PAGE_ALLOCATOR.lock().deallocate(block)
        } else {
            Err(InvalidArgument)
        }
    }


    #[allow(rustdoc::private_intra_doc_links)]
    /// Отмечает заданный блок виртуальных страниц как используемый.
    /// Ни выделения физической памяти, ни создания отображения станиц, не происходит.
    ///
    /// Возвращает ошибки:
    ///   - [`Error::NoPage`] если заданный блок не является целиком свободным.
    ///   - [`Error::PermissionDenied`] если принадлежность пользователю
    ///     какой-нибудь страницы блока не соответствует запрошенным флагам.
    ///     То есть, адрес этой страницы лежит в зарезервированной для пользователя области
    ///     [`kernel::memory::user_pages()`], а во `flags` нет доступа пользователю ---
    ///     флага [`PageTableFlags::USER_ACCESSIBLE`].
    ///     Либо наоборот, флаг [`PageTableFlags::USER_ACCESSIBLE`] есть, но страница не
    ///     лежит в области [`kernel::memory::user_pages()`].
    pub fn reserve(&mut self, pages: Block<Page>, flags: PageTableFlags) -> Result<()> {
        if flags.contains(PageTableFlags::USER_ACCESSIBLE) {
            if super::is_user_block(pages) {
                self.user_page_allocator.reserve(pages)
            } else {
                Err(PermissionDenied)
            }
        } else if super::user_pages().is_disjoint(pages) {
            KERNEL_PAGE_ALLOCATOR.lock().reserve(pages)
        } else {
            Err(PermissionDenied)
        }
    }


    #[allow(rustdoc::private_intra_doc_links)]
    /// Отображает заданную виртуальную страницу `page` на заданный физический фрейм `frame`
    /// с указанными флагами доступа `flags`.
    ///
    /// Если `page` уже была отображена, то старое отображение удаляется,
    /// а старый физический фрейм освобождается, если на него не осталось других ссылок.
    ///
    /// Возвращает ошибку:
    ///   - [`Error::PermissionDenied`] если принадлежность пользователю
    ///     страницы `page` не соответствует запрошенным флагам.
    ///     То есть адрес этой страницы лежит в зарезервированной для пользователя области
    ///     [`kernel::memory::user_pages()`], а во `flags` нет доступа пользователю ---
    ///     флага [`PageTableFlags::USER_ACCESSIBLE`].
    ///     Либо наоборот, флаг [`PageTableFlags::USER_ACCESSIBLE`] есть, но страница не
    ///     лежит в области [`kernel::memory::user_pages()`].
    ///
    /// # Safety
    ///
    /// Вызывающий код должен гарантировать, что инварианты управления памятью в Rust'е
    /// не будут нарушены.
    /// В частности, не осталось ссылок, которые ведут в страницу `page`.
    pub unsafe fn map_page_to_frame(
        &mut self,
        page: Page,
        frame: Frame,
        flags: PageTableFlags,
    ) -> Result<()> {
        if (flags.contains(PageTableFlags::USER_ACCESSIBLE) && !is_user_page(page)) || !(flags.contains(PageTableFlags::USER_ACCESSIBLE) && is_user_page(page)) {
            return Err(PermissionDenied);
        }
        let mut path = Mapping::path(self.mapping(), page.address());
        let old_entry = path.get();
            if old_entry.clone()?.present() {
                unsafe { path.unmap().expect("Failed to unmap"); }
                if let Ok(old_frame) = old_entry?.frame() {
                    let mut alloc = FRAME_ALLOCATOR.lock();
                    alloc.deallocate(old_frame);
                }
            }

        path.map_intermediate(flags).expect("Failed to allocate");
        let entry = path.get_mut();
            entry?.set_frame(frame, flags);

        Ok(())
    }


    /// Аллоцирует физический фрейм и отображает на него заданную виртуальную страницу `page`
    /// с указанными флагами доступа `flags`.
    /// Подробнее см. [`AddressSpace::map_page_to_frame()`].
    ///
    /// # Safety
    ///
    /// Вызывающий код должен гарантировать, что инварианты управления памятью в Rust'е
    /// не будут нарушены.
    /// В частности, не осталось ссылок, которые ведут в страницу `page`.
    pub(crate) unsafe fn map_page(&mut self, page: Page, flags: PageTableFlags) -> Result<Frame> {
        let mut alloc = FRAME_ALLOCATOR.lock();
        let frame = alloc.allocate().expect("Faild to allocate");
        let mut path = Mapping::path(self.mapping(), page.address());
        let old_entry = path.get();
        if old_entry.clone()?.present() {
            unsafe { path.unmap().expect("Failed to unmap"); }
            if let Ok(old_frame) = old_entry?.frame() {
                alloc.deallocate(old_frame);
            }
        }

        path.map_intermediate(flags).expect("Failed to allocate");
        let entry = path.get_mut();
        entry?.set_frame(frame, flags);

        Ok(frame)
    }


    /// Удаляет отображение заданной виртуальной страницы `page`.
    /// Физический фрейм освобождается, если на него не осталось других ссылок.
    ///
    /// # Safety
    ///
    /// Вызывающий код должен гарантировать, что инварианты управления памятью в Rust'е
    /// не будут нарушены.
    /// В частности, не осталось ссылок, которые ведут в удаляемую страницу.
    pub unsafe fn unmap_page(&mut self, page: Page) -> Result<()> {
        let virt = page.address();
        let mut path = self.mapping.path(virt);

        unsafe { path.unmap() }
    }


    /// Аллоцирует нужное количество физических фреймов
    /// и отображает в них заданный блок виртуальных страниц `pages`
    /// с заданными флагами доступа `flags`.
    ///
    /// # Safety
    ///
    /// Вызывающий код должен гарантировать, что инварианты управления памятью в Rust'е
    /// не будут нарушены.
    /// В частности, не осталось ссылок, которые ведут в `pages`.
    pub unsafe fn map_block(&mut self, pages: Block<Page>, flags: PageTableFlags) -> Result<()> {
        for page in pages {
            unsafe {
                self.map_page(page, flags)?;
            }
        }

        Ok(())
    }


    /// Меняет флаги доступа к заданному блоку виртуальных страниц `pages` на `flags`.
    ///
    /// # Safety
    ///
    /// Вызывающий код должен гарантировать, что инварианты управления памятью в Rust'е
    /// не будут нарушены.
    /// В частности, не осталось ссылок, которые ведут в `pages`.
    pub unsafe fn remap_block(&mut self, pages: Block<Page>, flags: PageTableFlags) -> Result<()> {
        for page in pages {
            self.mapping
                .path(page.address())
                .get_mut()?
                .set_flags(flags | PageTableFlags::PRESENT);
        }

        Ok(())
    }


    /// Удаляет отображение заданного блока виртуальных страниц `pages`.
    /// Физические фреймы, на которые не осталось других ссылок, освобождаются.
    ///
    /// # Safety
    ///
    /// Вызывающий код должен гарантировать, что инварианты управления памятью в Rust'е
    /// не будут нарушены.
    /// В частности, не осталось ссылок, которые ведут в `pages`.
    pub unsafe fn unmap_block(&mut self, pages: Block<Page>) -> Result<()> {
        for page in pages {
            unsafe {
                self.unmap_page(page)?;
            }
        }

        Ok(())
    }


    /// Аллоцирует нужное количество физических фреймов
    /// и отображает в них срез элементов типа `T` заданного размера `len`
    /// с заданными флагами доступа `flags`.
    ///
    /// Возвращает срез в виде неинициализированной памяти.
    fn map_slice_uninit<T>(
        &mut self,
        len: usize,
        flags: PageTableFlags,
    ) -> Result<&'static mut [MaybeUninit<T>]> {
        if Page::SIZE % mem::align_of::<T>() != 0 {
            warn!(
                alignment = mem::align_of::<T>(),
                page_size = Page::SIZE,
                "can not handle alignments greater than the page size"
            );
            return Err(WrongAlignment);
        }

        let block = self.allocate(len * mem::size_of::<T>(), flags)?;

        unsafe {
            self.map_block(block, flags)?;
        }

        let slice = unsafe { block.try_into_mut_slice()? };

        trace!(
            %block,
            page_count = block.count(),
            slice = format_args!("[{}; {}]", any::type_name::<T>(), slice.len()),
            "mapped a slice",
        );

        Ok(slice)
    }


    /// Аллоцирует нужное количество физических фреймов
    /// и отображает в них срез элементов типа `T` заданного размера `len`
    /// с заданными флагами доступа `flags`.
    ///
    /// Инициализирует все элементы среза лямбдой `default`.
    pub fn map_slice<T, F: Fn() -> T>(
        &mut self,
        len: usize,
        flags: PageTableFlags,
        default: F,
    ) -> Result<&'static mut [T]> {
        let slice = self.map_slice_uninit(len, flags)?;
        Ok(MaybeUninit::fill_with(slice, default))
    }


    /// Аллоцирует нужное количество физических фреймов
    /// и отображает в них срез элементов типа `T` заданного размера `len`
    /// с заданными флагами доступа `flags`.
    ///
    /// Инициализирует память нулями.
    ///
    /// # Safety
    ///
    /// Нулевой битовый паттерн должен быть валидным значением типа `T`,
    /// см. [`MaybeUninit::zeroed()`].
    pub unsafe fn map_slice_zeroed<T>(
        &mut self,
        len: usize,
        flags: PageTableFlags,
    ) -> Result<&'static mut [T]> {
        let slice = self.map_slice_uninit(len, flags)?;

        for element in slice.iter_mut() {
            *element = MaybeUninit::zeroed();
        }

        Ok(unsafe { MaybeUninit::slice_assume_init_mut(slice) })
    }


    /// Удаляет отображение заданного среза `slice`.
    /// Физические фреймы, на которые не осталось других ссылок, освобождаются.
    ///
    /// # Safety
    ///
    /// - Срез должен был быть ранее выделен одним из методов `AddressSpace::map_slice*()`.
    /// - Также вызывающий код должен гарантировать,
    ///   что инварианты управления памятью в Rust'е не будут нарушены.
    ///   В частности, не осталось ссылок, которые ведут в `slice`.
    pub unsafe fn unmap_slice<T>(&mut self, slice: &mut [T]) -> Result<()> {
        let ptr_range = slice.as_ptr_range();
        let start = Page::new(Virt::from_ptr(ptr_range.start))?;
        let end = Page::new(Virt::from_ptr(ptr_range.end))?;

        let block = Block::new(start, end)?;
        let page_count = block.count();

        unsafe {
            for element in slice.iter_mut() {
                ptr::drop_in_place(element as *mut T);
            }

            self.unmap_block(block)?;
        }

        trace!(
            addr = %start.address(),
            page_count,
            slice = format_args!("[{}; {}]", any::type_name::<T>(), slice.len()),
            "unmapped a slice",
        );

        Ok(())
    }


    /// Аллоцирует нужное количество физических фреймов
    /// и отображает в них один элемент типа `T`
    /// с заданными флагами доступа `flags`.
    ///
    /// Инициализирует элемент лямбдой `default`.
    pub fn map_one<T, F: FnOnce() -> T>(
        &mut self,
        flags: PageTableFlags,
        default: F,
    ) -> Result<&'static mut T> {
        let slice = self.map_slice_uninit(1, flags)?;
        slice[0].write(default());
        Ok(unsafe { MaybeUninit::assume_init_mut(&mut slice[0]) })
    }


    /// Удаляет отображение заданного элемента `element`.
    /// Физические фреймы, на которые не осталось других ссылок, освобождаются.
    ///
    /// # Safety
    ///
    /// - Элемент должен был быть ранее выделен методом [`AddressSpace::map_one()`].
    ///   Для дополнительной проверки этого, требует время жизни `'static`.
    /// - Также вызывающий код должен гарантировать, что инварианты управления памятью
    ///   в Rust'е не будут нарушены.
    ///   В частности, не осталось ссылок, которые ведут в `element`.
    ///   Для дополнительной проверки этого, требует мутабельность ссылки.
    pub unsafe fn unmap_one<T>(&mut self, element: &'static mut T) -> Result<()> {
        unsafe { self.unmap_slice(Block::<Virt>::from_mut(element).try_into_mut_slice::<T>()?) }
    }


    /// Проверяет доступность блока виртуальной памяти `block` на чтение и
    /// соответствие заданным флагам доступа `flags`.
    /// Возвращает иммутабельный срез типа `T`, расположенный в блоке `block`.
    ///
    /// Это позволяет объединить в одно действие проверку доступа и упростить последующий доступ
    /// к памяти, которую процесс пользователя указал ядру в системном вызове.
    ///
    /// Возвращает ошибки:
    ///   - [`Error::NoPage`] если какая-нибудь страница блока `block` не отображена.
    ///   - [`Error::PermissionDenied`] если какая-нибудь страница отображена,
    ///     но не со всеми запрошенными флагами.
    pub(crate) fn check_permission<T>(
        &mut self,
        block: Block<Virt>,
        flags: PageTableFlags,
    ) -> Result<&'static [T]> {
        // TODO: your code here.
        unimplemented!();
    }


    /// Проверяет доступность блока виртуальной памяти `block` на запись и
    /// соответствие заданным флагам доступа `flags`.
    /// Возвращает мутабельный срез типа `T`, расположенный в блоке `block`.
    ///
    /// Это позволяет объединить в одно действие проверку доступа и упростить последующий доступ
    /// к памяти, которую процесс пользователя указал ядру в системном вызове.
    ///
    /// Возвращает ошибки:
    ///   - [`Error::NoPage`] если какая-нибудь страница блока `block` не отображена.
    ///   - [`Error::PermissionDenied`] если какая-нибудь страница отображена,
    ///     но не со всеми запрошенными флагами.
    pub(crate) fn check_permission_mut<T>(
        &mut self,
        block: Block<Virt>,
        flags: PageTableFlags,
    ) -> Result<&'static mut [T]> {
        // TODO: your code here.
        unimplemented!();
    }


    /// Вспомогательный метод для
    /// [`AddressSpace::check_permission()`] и [`AddressSpace::check_permission_mut()`].
    /// Проверяет блок виртуальной памяти `block` на соответствие заданным флагам доступа `flags`.
    ///
    /// Возвращает ошибки:
    ///   - [`Error::NoPage`] если какая-нибудь страница блока `block` не отображена.
    ///   - [`Error::PermissionDenied`] если какая-нибудь страница отображена,
    ///     но не со всеми запрошенными флагами.
    fn check_permission_common(
        &mut self,
        block: &Block<Virt>,
        flags: PageTableFlags,
    ) -> Result<()> {
        // TODO: your code here.
        unimplemented!();
    }


    /// Выводит в лог карту виртуального адресного пространства.
    pub(crate) fn dump(&mut self) {
        debug!("address space:");

        let flag_mask = !(PageTableFlags::ACCESSED | PageTableFlags::DIRTY);
        let ignore_frame_addresses = true;

        for block in self.mapping()
            .iter_mut()
            .map(|path| path.block().unwrap())
            .coalesce(|a, b| a.coalesce(b, ignore_frame_addresses, flag_mask))
        {
            debug!("    {}", block);
        }
    }
}


impl Drop for AddressSpace {
    fn drop(&mut self) {
        if Mapping::current_page_table_root() == self.page_table_root() {
            let base_address_space = BASE_ADDRESS_SPACE.lock();
            assert!(self.page_table_root() != base_address_space.page_table_root());
            base_address_space.switch_to();
            info!(address_space = %self, switch_to = %base_address_space, "drop the current address space");
        } else {
            info!(address_space = %self, "drop");
        }
    }
}


impl fmt::Display for AddressSpace {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let page_table_root = self.page_table_root().address();
        match self.kind {
            Kind::Invalid => write!(formatter, "\"invalid\" @ {}", page_table_root),
            Kind::Base => write!(formatter, "\"base\" @ {}", page_table_root),
            Kind::Process => match self.pid {
                Some(pid) => write!(formatter, "\"{}\" @ {}", pid, page_table_root),
                None => write!(formatter, "\"process\" @ {}", page_table_root),
            },
        }
    }
}


/// Тип виртуального адресного пространства для логирования.
#[derive(Debug, Default)]
enum Kind {
    /// Некорректное виртуальное адресное пространство.
    #[default]
    Invalid,

    /// Базовое виртуальное адресное пространство.
    Base,

    /// Виртуальное адресное пространство некоторого процесса.
    Process,
}


/// Базовое виртуальное адресное пространство.
///
/// Создаётся загрузчиком до старта ядра и донастраивается ядром.
/// Все остальные адресные пространства получаются из него и его копий
/// с помощью метода [`AddressSpace::duplicate()`].
#[allow(rustdoc::private_intra_doc_links)]
pub static BASE_ADDRESS_SPACE: Spinlock<AddressSpace> = Spinlock::new(AddressSpace::zero());


#[doc(hidden)]
pub(super) mod test_scaffolding {
    use crate::error::Result;

    use super::{
        super::{
            mmu::PageTableFlags,
            page_allocator::test_scaffolding::block,
            Block,
            Frame,
            Page,
            Virt,
        },
        AddressSpace,
        Mapping,
    };


    pub fn new_address_space(page_table_root: Frame, phys2virt: Page) -> AddressSpace {
        AddressSpace::new(page_table_root, phys2virt)
    }


    pub unsafe fn map_page(
        address_space: &mut AddressSpace,
        page: Page,
        flags: PageTableFlags,
    ) -> Result<Frame> {
        unsafe { address_space.map_page(page, flags) }
    }


    pub unsafe fn unmap_page(address_space: &mut AddressSpace, page: Page) -> Result<()> {
        unsafe { address_space.unmap_page(page) }
    }


    pub fn check_permission<T>(
        address_space: &mut AddressSpace,
        block: Block<Virt>,
        flags: PageTableFlags,
    ) -> Result<&'static [T]> {
        address_space.check_permission(block, flags)
    }


    pub fn check_permission_mut<T>(
        address_space: &mut AddressSpace,
        block: Block<Virt>,
        flags: PageTableFlags,
    ) -> Result<&'static mut [T]> {
        address_space.check_permission_mut(block, flags)
    }


    pub fn duplicate(address_space: &AddressSpace) -> Result<AddressSpace> {
        address_space.duplicate()
    }


    pub fn mapping(address_space: &mut AddressSpace) -> &mut Mapping {
        &mut address_space.mapping
    }


    pub fn page_allocator_block(address_space: &AddressSpace) -> Block<Page> {
        block(&address_space.user_page_allocator)
    }


    pub fn switch_to(address_space: &AddressSpace) {
        address_space.switch_to();
    }
}
