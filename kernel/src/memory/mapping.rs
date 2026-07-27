use core::{fmt, marker::PhantomData, mem::MaybeUninit, ptr::NonNull};

use duplicate::duplicate_item;
use itertools::Either;
use crate::{
    error::{
        Error::{NoPage, Unimplemented},
        Result,
    },
    log::trace,
};

use super::{frage::{Frame, Page}, mapped_block::MappedBlock, mmu::{
    self,
    PageTable,
    PageTableEntry,
    PageTableFlags,
    FULL_ACCESS,
    PAGE_OFFSET_BITS,
    PAGE_TABLE_ENTRY_COUNT,
    PAGE_TABLE_INDEX_BITS,
    PAGE_TABLE_INDEX_MASK,
    PAGE_TABLE_LEAF_LEVEL,
    PAGE_TABLE_LEVEL_COUNT,
    PAGE_TABLE_ROOT_LEVEL,
    USER_READ,
}, size, Block, FrameDeallocatorGuard, Virt, FRAME_ALLOCATOR};

// Used in docs.
#[allow(unused)]
use crate::error::Error;


/// Многоуровневая таблица страниц.
///
/// Фактически дерево большой арности, если игнорировать рекурсивные записи.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct Mapping {
    /// Фрейм с корневым узлом таблицы страниц.
    page_table_root: Frame,

    /// Первая страница замапленной полностью физической памяти.
    ///
    /// Через область, которая с неё начинается, можно работать с любым участком
    /// физической памяти, не отображая его предварительно в виртуальное адресное пространство.
    /// См. [`super::phys2virt_map()`].
    phys2virt: Page,

    /// Номер рекурсивной записи в таблице страниц корневого уровня.
    /// Либо [`usize::MAX`], если рекурсивное отображение страниц не настроено.
    recursive_mapping: usize,
}


impl Mapping {
    /// Запоминает корневой узел таблицы страниц `page_table_root`.
    /// И начало "окна", в которое отображена вся физическая память, --- `phys2virt`.
    pub(super) const fn new(page_table_root: Frame, phys2virt: Page) -> Self {
        Self {
            page_table_root,
            phys2virt,
            recursive_mapping: usize::MAX,
        }
    }


    /// Создаёт копию отображения виртуальных страниц в физические фреймы [`Mapping`],
    /// которая указывает на те же целевые физические фреймы,
    /// то есть разделяет отображённую память.
    /// Но при этом само отображение для копии и оригинала хранится в разных физических фреймах.
    /// Поэтому копия может быть модифицирована независимо от оригинала.
    pub(super) fn duplicate(&self) -> Result<Self> {
        let mut result = Self::new(Frame::default(), self.phys2virt);
        result.page_table_root =
            self.duplicate_page_table(&mut result, self.page_table_root, PAGE_TABLE_ROOT_LEVEL)?;
        Ok(result)
    }


    /// Возвращает `true` если [`Mapping`] действительно содержит
    /// отображение виртуальной памяти в физическую.
    pub(super) fn is_valid(&self) -> bool {
        self.page_table_root != Frame::default()
    }


    /// Возвращает корневой узел таблицы страниц отображения.
    pub(super) fn page_table_root(&self) -> Frame {
        self.page_table_root
    }


    /// Принимает на вход виртуальный адрес `virt`, который нужно транслировать.
    ///
    /// Возвращает ссылку на запись типа [`PageTableEntry`] в
    /// узле листьевого уровня таблицы страниц,
    /// соответствующую входному виртуальному адресу `virt`.
    ///
    /// Возвращает ошибки:
    ///   - [`Error::NoPage`] если промежуточного или нужного листьевого узла таблицы страниц нет.
    ///   - [`Error::Unimplemented`] если промежуточный узел таблицы страниц
    ///     имеет флаг [`PageTableFlags::HUGE_PAGE`].
    pub fn translate(&mut self, virt: Virt) -> Result<&mut PageTableEntry> {
        self.path(virt).get_mut()
    }


    #[allow(rustdoc::private_intra_doc_links)]
    /// Принимает на вход виртуальный адрес `virt`, который нужно транслировать.
    ///
    /// Возвращает максимально длинный отображённый в память префикс пути в дереве трансляции,
    /// соответствующий входному виртуальному адресу `virt`.
    /// То есть, последний из узлов [`Path::nodes`], который не равен [`Virt::default()`],
    /// может указывать на [`PageTableEntry`] со сброшенным флагом [`PageTableFlags::PRESENT`],
    /// но сама эта [`PageTableEntry`] присутствует в памяти.
    /// Если же `virt` отображён в память, то либо это 4KiB-ая страница и
    /// тогда все элементы [`Path::nodes`] не равны [`Virt::default()`],
    /// либо последний не равный [`Virt::default()`], элемент соответствует
    /// [`PageTableEntry`] с установленным флагом [`PageTableFlags::HUGE_PAGE`].
    pub fn path(&mut self, virt: Virt) -> Path {
        let mut nodes = [Virt::default(); PAGE_TABLE_LEVEL_COUNT];
        let mut frame = self.page_table_root();

        for (level, node) in nodes.iter_mut().enumerate().rev() {
            let pte = unsafe { self.pte_ref(virt, level as u32, frame) };
            *node = Virt::from_ref(pte);

            if pte.present() && !pte.flags().contains(PageTableFlags::HUGE_PAGE) {
                frame = pte
                    .frame()
                    .expect("PageTableEntry should point to a frame if PRESENT bit is set");
            } else {
                break;
            }
        }

        Path {
            mapping: self,
            nodes,
            virt,
        }
    }


    /// Возвращает итератор по листьям дерева отображения страниц.
    pub fn iter_mut(&mut self) -> MappingIterator {
        MappingIterator {
            _marker: PhantomData,
            mapping: self.into(),
            virt: Some(Virt::lower_half()),
        }
    }


    /// Шаг рекурсии при спуске по дереву отображения страниц.
    /// Выполняет основную работу по созданию копии отображения [`Mapping`],
    /// см. [`Mapping::duplicate()`].
    fn duplicate_page_table(
        &self,
        dst: &mut Mapping,
        src_frame: Frame,
        level: u32,
    ) -> Result<Frame> {
        let dst_frame = FrameDeallocatorGuard::new(FRAME_ALLOCATOR.lock().allocate()?);

        let src_pt = unsafe { self.page_table_ref(src_frame) };
        let dst_pt = unsafe {
            super::phys2virt_map(self.phys2virt, dst_frame.address())
                .try_into_mut::<PageTable>()?
        };

        let is_leaf = level == PAGE_TABLE_LEAF_LEVEL;
        let is_user_accessible_leaf =
            |pte: &PageTableEntry| is_leaf && pte.flags().contains(PageTableFlags::USER_ACCESSIBLE);

        dst_pt.clone_from(src_pt);

        dst_pt
            .iter_mut()
            .filter(|pte| is_user_accessible_leaf(pte))
            .for_each(|user_pte| *user_pte = PageTableEntry::default());

        for (src_pte, dst_pte) in src_pt
            .iter()
            .zip(dst_pt.iter_mut())
            .filter(|&(pte, _)| pte.present())
            .filter(|&(pte, _)| !pte.flags().contains(PageTableFlags::HUGE_PAGE))
            .filter(|&(pte, _)| !is_user_accessible_leaf(pte))
        {
            let src_frame = src_pte
                .frame()
                .expect("PageTableEntry should point to a frame if PRESENT bit is set");

            if is_leaf {
                FRAME_ALLOCATOR.lock().reference(src_frame);
            } else {
                let dst_child_frame = self.duplicate_page_table(dst, src_frame, level - 1)?;

                dst_pte.set_frame(dst_child_frame, src_pte.flags());
            }
        }

        Ok(dst_frame.take())
    }


    /// Шаг рекурсии при спуске по дереву отображения страниц.
    /// Выполняет основную работу по освобождению физических фреймов
    /// как отображённых [`Mapping`], так и занятых самим отображением.
    ///
    /// - `node` --- физический фрейм с текущим узлом;
    /// - `level` --- уровень текущего узла в дереве отображения страниц;
    /// - `drop_used` --- равен `true`, если нужно удалить все узлы дерева,
    ///   и `false`, если нужно удалить только узлы, которые фактически не нужны.
    ///   То есть те, через которые не ведут пути для отображённых страниц.
    ///
    /// Используется в [`Mapping::drop()`] и [`Mapping::unmap_unused_intermediate()`].
    fn drop_subtree(&mut self, node: Frame, level: u32, drop_used: bool) -> bool {
        let virt_pt = super::phys2virt_map(self.phys2virt, node.address());
        let pt = unsafe { virt_pt.try_into_mut::<PageTable>().unwrap() };

        let is_leaf = level == PAGE_TABLE_LEAF_LEVEL;

        let mut num_present;
        let pte_drop_list;

        if drop_used {
            num_present = 0;
            pte_drop_list = Either::Left(
                pt.iter_mut()
                    .filter(|pte| pte.present())
                    .filter(|pte| !pte.flags().contains(PageTableFlags::HUGE_PAGE)),
            );
        } else {
            num_present = pt.iter().filter(|pte| pte.present()).count();
            pte_drop_list = Either::Right(
                pt.iter_mut()
                    .filter(|_| !is_leaf)
                    .filter(|pte| pte.present())
                    .filter(|pte| !pte.flags().contains(PageTableFlags::HUGE_PAGE)),
            );
        };

        for pte in pte_drop_list {
            let frame = pte.frame().unwrap();

            if drop_used && is_leaf {
                FRAME_ALLOCATOR.lock().deallocate(frame);
            } else {
                let dropped = self.drop_subtree(frame, level - 1, drop_used);

                if dropped && !drop_used {
                    pte.clear();
                    num_present -= 1;
                }
            }
        }

        if drop_used || num_present == 0 {
            FRAME_ALLOCATOR.lock().deallocate(node);

            true
        } else {
            false
        }
    }


    /// Освобождает не использующиеся промежуточные узлы отображения страниц.
    pub fn unmap_unused_intermediate(&mut self) {
        if self.is_valid() && self.drop_subtree(self.page_table_root, PAGE_TABLE_ROOT_LEVEL, false)
        {
            self.page_table_root = Frame::default();
        }
    }


    /// Выбирает в таблице страниц корневого уровня свободную запись,
    /// которую инициализирует как рекурсивную.
    /// Сохраняет её номер в поле [`Mapping::recursive_mapping`] и
    /// возвращает его в вызывающую функцию.
    ///
    /// Если свободных записей в таблице страниц корневого уровня нет,
    /// возвращает ошибку [`Error::NoPage`].
    pub(crate) fn make_recursive_mapping(&mut self) -> Result<usize> {
        // TODO: your code here.
        Ok(self.recursive_mapping) // TODO: remove before flight.
    }


    /// Возвращает физический фрейм корневого узла текущего отображения
    /// виртуальной памяти в физическую.
    pub(crate) fn current_page_table_root() -> Frame {
        mmu::page_table_root()
    }


    /// Возвращает ссылку на узел таблицы страниц,
    /// записанный в данном физическом фрейме.
    ///
    /// # Safety
    ///
    /// Вызывающий код должен гарантировать, что:
    ///   - Во `frame` находится узел таблицы страниц.
    ///   - Инварианты управления памятью в Rust'е не будут нарушены.
    ///     В частности, нет других ссылок, которые ведут во `frame`.
    #[allow(clippy::needless_arbitrary_self_type)]
    #[allow(unused)]
    #[duplicate_item(
        page_table_getter reference(x) return_type;
        [page_table_ref] [&x] [PageTable];
        [page_table_mut] [&mut x] [PageTable];
        [page_table_uninit_mut] [&mut x] [MaybeUninit<PageTable>];
    )]
    pub(super) unsafe fn page_table_getter(
        self: reference([Self]),
        frame: Frame,
    ) -> reference([return_type]) {
        let page_table = super::phys2virt_map(self.phys2virt, frame.address());
        unsafe { page_table.try_into_mut().expect("bad phys2virt or frame") }
    }


    /// Возвращает ссылку на одну запись узла таблицы страниц,
    /// записанного в физическом фрейме `page_table_frame`.
    /// Запись соответствует виртуальному адресу `virt`,
    /// а `level` указывает уровень узла в дереве отображения.
    ///
    /// # Safety
    ///
    /// Вызывающий код должен гарантировать, что:
    ///   - Во `frame` находится узел таблицы страниц уровня `level`.
    ///   - Инварианты управления памятью в Rust'е не будут нарушены.
    ///     В частности, нет других ссылок, которые ведут в ту же запись [`PageTableEntry`].
    #[allow(clippy::needless_arbitrary_self_type)]
    #[allow(unused)]
    #[duplicate_item(
        getter page_table_getter reference(x);
        [pte_ref] [page_table_ref] [&x];
        [pte_mut] [page_table_mut] [&mut x];
    )]
    unsafe fn getter(
        self: reference([Self]),
        virt: Virt,
        level: u32,
        page_table_frame: Frame,
    ) -> reference([PageTableEntry]) {
        let index_shift = PAGE_OFFSET_BITS + level * PAGE_TABLE_INDEX_BITS;
        let index = (virt.into_usize() >> index_shift) & PAGE_TABLE_INDEX_MASK;
        let page_table = unsafe { self.page_table_getter(page_table_frame) };

        reference([page_table[index]])
    }


    /// Принимает промежуточную запись [`PageTableEntry`],
    /// ссылающуюся на отсутствующий узел дерева отображения.
    /// Выделяет с помощью [`static@FRAME_ALLOCATOR`] фрейм и записывает его
    /// в эту [`PageTableEntry`] вместе с флагом [`PageTableFlags::PRESENT`].
    ///
    /// См. [`Path::map_intermediate()`] в котором этот метод
    /// используется как вспомогательный.
    #[allow(unused)]
    fn map_intermediate(&mut self, pte: &mut PageTableEntry) -> Result<()> {
        let frame = FRAME_ALLOCATOR.lock().allocate()?;

        unsafe { self.page_table_mut(frame).fill(PageTableEntry::default()) };

        pte.set_frame(frame, PageTableFlags::empty());

        Ok(())
    }


    /// Создаёт новый узел --- [`PageTable`] --- дерева отображения страниц,
    /// но не провязывает его в дерево.
    ///
    /// Очищает записи [`PageTableEntry`] в новом узле с помощью [`MaybeUninit::zeroed()`].
    /// Возвращает фрейм нового узла.
    fn allocate_node(&mut self) -> Result<Frame> {
        // TODO: your code here.
        unimplemented!();
    }
}


impl Drop for Mapping {
    fn drop(&mut self) {
        assert!(Self::current_page_table_root() != self.page_table_root);

        if self.is_valid() {
            self.drop_subtree(self.page_table_root, PAGE_TABLE_ROOT_LEVEL, true);
        }
    }
}


/// Путь в дереве отображения заданного виртуального адреса.
#[derive(Debug, Eq, PartialEq)]
pub struct Path<'a> {
    /// Дерево отображения.
    mapping: &'a mut Mapping,

    /// Узлы на пути в дереве отображения,
    /// задаваемые как адреса соответствующих [`PageTableEntry`].
    /// Элемент с индексом [`PAGE_TABLE_LEAF_LEVEL`] задаёт адрес [`PageTableEntry`],
    /// отображающей виртуальную страницу адреса [`Path::virt`] на его физическую страницу.
    /// Элемент с индексом [`PAGE_TABLE_ROOT_LEVEL`] задаёт адрес [`PageTableEntry`],
    /// находящейся в корневой таблице страниц.
    /// Если какой-то из узлов не отображён в память,
    /// соответствующий элемент равен [`Virt::default()`].
    nodes: [Virt; PAGE_TABLE_LEVEL_COUNT],

    /// Адрес, для которого построен путь отображения.
    virt: Virt,
}


impl<'a> Path<'a> {
    /// Создаёт путь в дереве отображения заданного виртуального адреса.
    fn new(mapping: &'a mut Mapping, nodes: [Virt; PAGE_TABLE_LEVEL_COUNT], virt: Virt) -> Self {
        Self {
            mapping,
            nodes,
            virt,
        }
    }


    #[allow(rustdoc::private_intra_doc_links)]
    /// Выделяет физические фреймы под отсутствующие промежуточные таблицы страничного отображения.
    /// И исправляет флаги с которыми они отображены так, чтобы целевой адрес [`Path::virt`]
    /// можно было отобразить с эффективными флагами `flags`.
    ///
    /// Возвращает ошибки:
    ///   - [`Error::NoFrame`] если пришлось выделить физический фрейм,
    ///     но их не осталось во [`static@FRAME_ALLOCATOR`].
    ///   - [`Error::Unimplemented`] если промежуточный узел таблицы страниц
    ///     имеет флаг [`PageTableFlags::HUGE_PAGE`].
    pub fn map_intermediate(&mut self, flags: PageTableFlags) -> Result<()> {
        let mut nodes_iter = self.nodes.iter_mut().enumerate().rev().peekable();

        while let Some((level, node)) = nodes_iter.next() {
            let pte: &mut PageTableEntry = unsafe { node.try_into_mut().unwrap() };

            if pte.present() && pte.flags().contains(PageTableFlags::HUGE_PAGE) {
                return Err(Error::Unimplemented);
            }

            if !pte.present() && level != 0 {
                self.mapping.map_intermediate(pte)?;

                if let Some((next_level, next_node)) = nodes_iter.peek_mut() {
                    let next_pte = unsafe {
                        self.mapping.pte_ref(
                            self.virt,
                            *next_level as u32,
                            pte.frame().expect(
                                "Mapping::map_intermediate should have allocated and set frame",
                            ),
                        )
                    };

                    **next_node = Virt::from_ref(next_pte);
                }
            }

            pte.set_flags(
                if level != 0 {
                    (pte.flags() | flags) & FULL_ACCESS
                } else {
                    PageTableFlags::empty()
                },
            );
        }

        Ok(())
    }


    #[allow(rustdoc::private_intra_doc_links)]
    /// Возвращает листьевую [`PageTableEntry`], которая отвечает за отображение адреса [`Path::virt`].
    ///
    /// Возвращает ошибки:
    ///   - [`Error::NoPage`] если промежуточного или нужного листьевого узла таблицы страниц нет.
    ///   - [`Error::Unimplemented`] если промежуточный узел таблицы страниц
    ///     имеет флаг [`PageTableFlags::HUGE_PAGE`].
    #[allow(clippy::needless_arbitrary_self_type)]
    #[duplicate_item(
        getter self_type return_type deepest_pte_getter;
        [get] [&Self] [&'a PageTableEntry] [deepest_pte];
        [get_mut] [&mut Self] [&'a mut PageTableEntry] [deepest_pte_mut];
    )]
    pub fn getter(self: self_type) -> Result<return_type> {
        let (level, pte) = self.deepest_pte_getter();

        if level == 0 {
            Ok(pte)
        } else if pte.flags().contains(PageTableFlags::HUGE_PAGE) {
            Err(Unimplemented)
        } else {
            Err(NoPage)
        }
    }


    /// Возвращает отображение блока виртуальных страниц
    /// на блок физических фреймов, если текущий [`Path`]
    /// задаёт путь к листу дерева отображения.
    /// Возвращает [`None`], если в текущем [`Path`]
    /// отсутствует часть промежуточных узлов.
    pub fn block(&self) -> Option<MappedBlock> {
        let (level, pte) = self.deepest_pte();
        let frage_count = PAGE_TABLE_ENTRY_COUNT.pow(level);

        if let Ok(frame) = pte.frame() &&
            (level == 0 || pte.flags().contains(PageTableFlags::HUGE_PAGE))
        {
            let page = Page::containing(self.virt);
            let message = "block is not ready for some frage ranges";

            let flags = pte.flags();
            let frames = Block::new(frame, (frame + frage_count).expect(message)).expect(message);
            let pages = Block::new(page, (page + frage_count).expect(message)).expect(message);

            Some(MappedBlock::new(flags, frames, pages))
        } else {
            None
        }
    }


    /// Удаляет отображение страницы по текущему пути.
    /// Физический фрейм освобождается, если на него не осталось других ссылок.
    ///
    /// # Safety
    ///
    /// Вызывающий код должен гарантировать, что инварианты управления памятью в Rust'е
    /// не будут нарушены.
    /// В частности, не осталось ссылок, которые ведут в удаляемую страницу.
    pub(super) unsafe fn unmap(&mut self) -> Result<()> {
        let pte = self.get_mut()?;
        let frame = pte.frame()?;
        FRAME_ALLOCATOR.lock().deallocate(frame);
        pte.clear();
        let page = Page::containing(self.virt);
        unsafe {mmu::flush(page);}
        Ok(())
    }


    /// Возвращает самую далёкую от корня дерева [`PageTableEntry`]
    /// в данном [`Path`] вместе с её номером уровня в дереве отображения.
    #[allow(clippy::needless_arbitrary_self_type)]
    #[duplicate_item(
        deepest_pte_getter self_type return_type converter;
        [deepest_pte] [&Self] [&'a PageTableEntry] [try_into_ref];
        [deepest_pte_mut] [&mut Self] [&'a mut PageTableEntry] [try_into_mut];
    )]
    fn deepest_pte_getter(self: self_type) -> (u32, return_type) {
        let (level, node) = self
            .nodes
            .iter()
            .enumerate()
            .find(|&(_, &node)| node != Virt::default())
            .expect("valid path should have valid root page table entry");

        let level = level.try_into().expect("unreasonable PTE level");
        let pte = unsafe { node.converter().expect("corrupted path") };

        (level, pte)
    }
}


impl<'a> fmt::Display for Path<'a> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        for level in (PAGE_TABLE_LEAF_LEVEL..=PAGE_TABLE_ROOT_LEVEL).rev() {
            write!(formatter, "L{}: {}", level, self.nodes[size::from(level)])?;

            let pte = unsafe { self.nodes[size::from(level)].try_into_ref::<PageTableEntry>() };

            if let Ok(pte) = pte {
                if pte.present() {
                    if pte.flags().contains(PageTableFlags::HUGE_PAGE) {
                        return write!(formatter, " (huge)");
                    }
                } else {
                    return write!(formatter, " (non-present)");
                }
            } else {
                return Ok(());
            }

            if level != PAGE_TABLE_LEAF_LEVEL {
                write!(formatter, " -> ")?;
            }
        }

        let pte = unsafe {
            self.nodes[size::from(PAGE_TABLE_LEAF_LEVEL)].try_into_ref::<PageTableEntry>()
        };

        if let Ok(pte) = pte &&
            let Ok(frame) = pte.frame()
        {
            let offset = (self.virt - Page::containing(self.virt).address())
                .expect("virt offset in its page is always valid");
            let phys = (frame.address() + offset).expect("offset inside a frame is always valid");
            write!(formatter, " => {}", phys)
        } else {
            Ok(())
        }
    }
}


/// Итератор по листьям дерева отображения страниц.
pub struct MappingIterator<'a> {
    /// Маркер, привязывающий время жизни [`MappingIterator`]
    /// ко времени жизни соответствующего [`Mapping`].
    _marker: PhantomData<&'a mut Mapping>,

    /// Дерево отображения.
    mapping: NonNull<Mapping>,

    /// Текущий виртуальный адрес, задающий позицию итератора,
    /// или [`None`], если достигнут конец.
    virt: Option<Virt>,
}


impl<'a> Iterator for MappingIterator<'a> {
    type Item = Path<'a>;


    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let virt = self.virt?;

            let path = unsafe { (*self.mapping.as_ptr()).path(virt) };

            let root_level = size::from(PAGE_TABLE_ROOT_LEVEL);
            if path.nodes[root_level] == path.nodes[root_level - 1] {
                let frage_count = PAGE_TABLE_ENTRY_COUNT.pow(PAGE_TABLE_ROOT_LEVEL);
                self.virt = (virt + frage_count * Page::SIZE).ok();
                continue;
            }

            let (level, pte) = path.deepest_pte();
            let frage_count = PAGE_TABLE_ENTRY_COUNT.pow(level);

            self.virt = (virt + frage_count * Page::SIZE).ok().or_else(|| {
                if virt.is_lower_half() {
                    Some(Virt::higher_half())
                } else {
                    None
                }
            });

            if pte.present() && (level == 0 || pte.flags().contains(PageTableFlags::HUGE_PAGE)) {
                return Some(path);
            }
        }
    }
}


#[doc(hidden)]
pub(super) mod test_scaffolding {
    use duplicate::duplicate_item;

    use super::super::{
        mmu::{PageTable, PageTableEntry, PAGE_TABLE_LEVEL_COUNT},
        Page,
        Virt,
    };

    pub use super::{Mapping, Path};


    pub fn deepest_pte<'a>(path: &'a Path<'a>) -> (u32, &'a PageTableEntry) {
        path.deepest_pte()
    }


    pub fn nodes(path: &Path) -> [Virt; PAGE_TABLE_LEVEL_COUNT] {
        path.nodes
    }


    #[allow(clippy::needless_arbitrary_self_type)]
    #[allow(unused)]
    #[duplicate_item(
        page_table_root_getter page_table_getter reference(x);
        [page_table_root] [page_table_ref] [&x];
        [page_table_root_mut] [page_table_mut] [&mut x];
    )]
    pub fn page_table_root_getter(mapping: reference([Mapping])) -> reference([PageTable]) {
        unsafe { mapping.page_table_getter(mapping.page_table_root()) }
    }


    pub fn phys2virt(mapping: &Mapping) -> Page {
        mapping.phys2virt
    }
}
