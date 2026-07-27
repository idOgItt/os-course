use core::{marker::PhantomData, ptr::NonNull};
use core::ops::{Deref, DerefMut, Index};
use duplicate::duplicate_item;
use core::option::Option;
use tracing::{debug, error, info, warn};
use ku::memory::Phys;
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
}, size, Block, FrameAllocator, Virt, FRAME_ALLOCATOR};

// Used in docs.
#[allow(unused)]
use crate::error::Error;
use crate::memory::test_scaffolding::phys2virt;

/// Многоуровневая таблица страниц.
///
/// Фактически дерево большой арности, если игнорировать рекурсивные записи.
#[derive(Debug, Default)]
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
        let mut nodes: [Virt; PAGE_TABLE_LEVEL_COUNT] = [Virt::default(); PAGE_TABLE_LEVEL_COUNT];
        let mut current_frame = self.page_table_root();


        for level in (PAGE_TABLE_LEAF_LEVEL..=PAGE_TABLE_ROOT_LEVEL).rev() {
            // let index_shift = PAGE_OFFSET_BITS + level * PAGE_TABLE_INDEX_BITS;
            // let index = (virt.into_usize() >> index_shift) & PAGE_TABLE_INDEX_MASK;
            let entry = unsafe { self.pte_ref(virt, level, current_frame) };
            let flags = entry.flags();

            nodes[level as usize] = Virt::from_ref(entry);

            if flags.contains(PageTableFlags::HUGE_PAGE) || !flags.contains(PageTableFlags::PRESENT) {
                break;
            }

            if let Ok(frame) = entry.frame() {
                current_frame = frame;
            } else {
                break;
            }
        }
        let path = Path::new(self, nodes, virt);
        path
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
        // debug!("Dublication start: {:?} {:?} {:?}", src_frame, level, dst);
        let mut alloc = FRAME_ALLOCATOR.lock();
        let new_frame = alloc.allocate()?;
        drop(alloc);

        let src_page_table = unsafe { self.page_table_ref(src_frame) };
        let mut dst_page_table: [PageTableEntry; 512] = {
            let page_table = super::phys2virt_map(self.phys2virt, new_frame.address());
            unsafe { *page_table.try_into_mut().expect("Bad frame") }
        };

        dst_page_table.clone_from(src_page_table);
        for (i, src_entry) in src_page_table.iter().enumerate() {
            if !src_entry.present() {
                continue;
            }
            if  level == PAGE_TABLE_LEAF_LEVEL {
                let mut alloc = FRAME_ALLOCATOR.lock();
                if !src_entry.flags().contains(PageTableFlags::USER_ACCESSIBLE) {
                    alloc.reference(src_frame);
                }
                drop(alloc);
                dst_page_table[i] = *src_entry;
            } else {
                let dst_sub_frame = {
                    let src_sub_frame = src_entry.frame()?;
                    self.duplicate_page_table(dst, src_sub_frame, level - 1)?
                };
                // debug!("Src_entry start: {:?}", src_entry);
                dst_page_table[i].set_frame(dst_sub_frame, src_entry.flags());
            }
        }
        // debug!("Dublication done: {:?}", new_frame);
        Ok(new_frame)
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

        debug!("Drop sub_tree start: {:?} {:?} {:?}", node, level, drop_used);

        let mut all_entries_empty = true;
        let mut page_table :  [ku::memory::mmu::PageTableEntry; 512]  = unsafe { *self.page_table_mut(node) };
        for i in 0..2 {
            debug!("Drop subtree cycle: {:?}", i);
            all_entries_empty = true;
            for (index, src_entry) in page_table.iter_mut().enumerate() {
                if src_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
                    all_entries_empty = false;
                    continue;
                }

                if !src_entry.present() {
                    continue;
                }

                    if level > PAGE_TABLE_LEAF_LEVEL {
                        debug!("Drop sub_tree start: {:?} {:?}", node, level);
                        if let Ok(sub_frame) = src_entry.frame() {
                            let subtree_empty = {
                                self.drop_subtree(sub_frame, level - 1, drop_used)
                            };
                            if !subtree_empty {
                                all_entries_empty = false;
                            } else {
                                src_entry.set_flags(src_entry.flags() & !PageTableFlags::PRESENT);
                            }
                        }
                    } else {
                        // L0 - if drop_used == true --> delete
                        if drop_used {
                            if let Ok(frame) = src_entry.frame() {
                                let mut alloc = FRAME_ALLOCATOR.lock();
                                alloc.deallocate(node);
                                drop(alloc);
                            }
                        }
                        all_entries_empty = false;
                    }
            }
        }
        if all_entries_empty || drop_used {
            let mut alloc = FRAME_ALLOCATOR.lock();
            alloc.deallocate(node);
            drop(alloc);
            return true;
        }

        false
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


    /// Возвращает иммутабельную ссылку на узел таблицы страниц,
    /// записанный в данном физическом фрейме.
    ///
    /// # Safety
    ///
    /// Вызывающий код должен гарантировать, что:
    ///   - Во `frame` находится узел таблицы страниц.
    ///   - Инварианты управления памятью в Rust'е не будут нарушены.
    ///     В частности, нет мутабельных ссылок, которые ведут во `frame`.
    pub(super) unsafe fn page_table_ref(&self, frame: Frame) -> &PageTable {
        let page_table = super::phys2virt_map(self.phys2virt, frame.address());
        unsafe { page_table.try_into_ref().expect("bad phys2virt or frame") }
    }


    /// Возвращает мутабельную ссылку на узел таблицы страниц,
    /// записанный в данном физическом фрейме.
    ///
    /// # Safety
    ///
    /// Вызывающий код должен гарантировать, что:
    ///   - Во `frame` находится узел таблицы страниц.
    ///   - Инварианты управления памятью в Rust'е не будут нарушены.
    ///     В частности, нет других ссылок, которые ведут во `frame`.
    unsafe fn page_table_mut(&mut self, frame: Frame) -> &mut PageTable {
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
    fn map_intermediate(&mut self, pte: &mut PageTableEntry) -> Result<()> {
        debug!("Starting Mapping::map_intermediate with PageTableEntry: {:?}", pte);
        let mut alloc = FRAME_ALLOCATOR.lock();
        let frame = alloc.allocate()?;

        unsafe { self.page_table_mut(frame).fill(PageTableEntry::default()) };

        pte.set_frame(frame, pte.flags());

        debug!("Allocation done: {:?}", pte);

        Ok(())
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
        debug!("Starting map_intermediate with flags: {:?}", flags);
        while let Some((level, node)) = nodes_iter.next() {
            let pte: &mut PageTableEntry = unsafe { node.try_into_mut().unwrap() };
            debug!("Processing node at level {}: {:?}", level, pte);
            if pte.flags().contains(PageTableFlags::HUGE_PAGE) {
                error!("Encountered HUGE_PAGE at level {}", level);
                return Err(Unimplemented);
            }

            if !pte.present() && level != 0 {
                debug!("Node is not  present. Calling map_intermediate :: mapping {}", level);
                self.mapping.map_intermediate(pte)?;
            }

            if let Some((next_level, next_node)) = nodes_iter.peek_mut() {
                debug!("Peeking next node at level {}", next_level);
                let next_pte = unsafe {
                    self.mapping.pte_ref(
                        self.virt,
                        *next_level as u32,
                        pte.frame().expect("Mapping::map_intermediate should have allocated and set frame"),
                    )
                };
                debug!("Next PTE retrieved: {:?}", next_pte);
                **next_node = Virt::from_ref(next_pte);
            }

            pte.set_flags(
                if level != 0 {
                    debug!("Setting flags for non-zero level");
                    pte.flags() | (flags & FULL_ACCESS)
                } else {
                    debug!("Setting flags for level 0");
                    PageTableFlags::empty()
                }
            );
        }

        info!("map_intermediate completed successfully");

        Ok(())
    }


    /// Возвращает листьевую [`PageTableEntry`], которая отвечает за отображение адреса [`Path::virt`].
    ///
    /// Если промежуточного или нужного листьевого узла таблицы страниц нет,
    /// возвращает ошибку [`Error::NoPage`].
    #[allow(clippy::needless_arbitrary_self_type)]
    #[duplicate_item(
        getter self_type return_type converter;
        [get] [&Self] [&'a PageTableEntry] [try_into_ref];
        [get_mut] [&mut Self] [&'a mut PageTableEntry] [try_into_mut];
    )]
    pub fn getter(self: self_type) -> Result<return_type> {
        unsafe {
            self.nodes[size::from(PAGE_TABLE_LEAF_LEVEL)]
                .converter::<PageTableEntry>()
                .map_err(|_| NoPage)
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
        // TODO: your code here.
        unimplemented!();
    }


    /// Возвращает самую далёкую от корня дерева [`PageTableEntry`]
    /// в данном [`Path`] вместе с её номером уровня в дереве отображения.
    fn deepest_pte(&self) -> (u32, &PageTableEntry) {
        let (level, node) = self
            .nodes
            .iter()
            .enumerate()
            .find(|&(_, &node)| node != Virt::default())
            .expect("valid path should have valid root page table entry");

        let level = level.try_into().expect("unreasonable PTE level");
        let pte = unsafe { node.try_into_mut::<PageTableEntry>().expect("corrupted path") };

        (level, pte)
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
    use super::super::{mmu::PageTable, Page};

    pub use super::Mapping;


    pub fn page_table_root(mapping: &mut Mapping) -> &mut PageTable {
        unsafe { mapping.page_table_mut(mapping.page_table_root()) }
    }


    pub fn phys2virt(mapping: &Mapping) -> Page {
        mapping.phys2virt
    }
}
