use core::{
    fmt,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::{Bound, RangeBounds},
    ptr::NonNull,
};

use bitflags::Flags;
use duplicate::duplicate_item;
use itertools::Either;
use ku::dbg;

use crate::{
    error::{
        Error::{InvalidArgument, NoPage, Unimplemented},
        Result,
    },
    log::{debug, error, trace},
};

use super::{
    frage::{Frame, Page},
    mapped_block::MappedBlock,
    mmu::{
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
    },
    size,
    Block,
    FrameDeallocatorGuard,
    Phys2Virt,
    Virt,
    FRAME_ALLOCATOR,
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
use crate::memory::test_scaffolding::phys2virt;

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
    phys2virt: Page,

    /// Для простоты работы с физической памятью,
    /// она целиком линейно отображена в некоторую область виртуальной.
    /// [`Mapping::phys2virt_mapping`] описывает это отображение.
    phys2virt_mapping: Phys2Virt,

    /// Номер рекурсивной записи в таблице страниц корневого уровня.
    /// Либо [`usize::MAX`], если рекурсивное отображение страниц не настроено.
    recursive_mapping: usize,
}


impl Mapping {
    /// Инициализирует страничное отображение по корневому узлу `page_table_root`.
    /// Аргумент [`phys2virt`][Phys2Virt] описывает линейное отображение
    /// физической памяти в виртуальную внутри этого страничного отображения.
    pub(super) fn new(page_table_root: Frame, phys2virt: Phys2Virt) -> Self {
        Self {
            page_table_root,
            phys2virt: phys2virt.start(),
            phys2virt_mapping: phys2virt,
            recursive_mapping: usize::MAX,
        }
    }


    /// Возвращает пустое отображение [`Mapping`].
    /// В отличие от [`Mapping::default()`] доступна в константном контексте.
    pub(super) const fn zero() -> Self {
        Self {
            page_table_root: Frame::zero(),
            phys2virt: Page::zero(),
            phys2virt_mapping: Phys2Virt::zero(),
            recursive_mapping: usize::MAX,
        }
    }


    /// Создаёт копию отображения виртуальных страниц в физические фреймы [`Mapping`],
    /// которая указывает на те же целевые физические фреймы,
    /// то есть разделяет отображённую память.
    /// Но при этом само отображение для копии и оригинала хранится в разных физических фреймах.
    /// Поэтому копия может быть модифицирована независимо от оригинала.
    pub(super) fn duplicate(&self) -> Result<Self> {
        let mut result = Self::new(Frame::default(), self.phys2virt_mapping);
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


    /// Возвращает линейное отображение физической памяти в виртуальную, см. [`Phys2Virt`].
    pub(super) fn phys2virt(&self) -> Phys2Virt {
        self.phys2virt_mapping
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
<<<<<<< HEAD
        let mut nodes: [Virt; PAGE_TABLE_LEVEL_COUNT] = [Virt::default(); PAGE_TABLE_LEVEL_COUNT];
        let mut current_frame = self.page_table_root();


        for level in (PAGE_TABLE_LEAF_LEVEL..=PAGE_TABLE_ROOT_LEVEL).rev() {
            // let index_shift = PAGE_OFFSET_BITS + level * PAGE_TABLE_INDEX_BITS;
            // let index = (virt.into_usize() >> index_shift) & PAGE_TABLE_INDEX_MASK;
            let entry = unsafe { self.pte_ref(virt, level, current_frame) };
            let flags = entry.flags();

            nodes[level as usize] = Virt::from_ref(entry);

            if flags.contains(PageTableFlags::HUGE_PAGE) || !flags.contains(PageTableFlags::PRESENT)
            {
                break;
            }

            if let Ok(frame) = entry.frame() {
                current_frame = frame;
=======
        let mut nodes = [Virt::default(); PAGE_TABLE_LEVEL_COUNT];
        let mut frame = self.page_table_root();

        for (level, node) in nodes.iter_mut().enumerate().rev() {
            let pte = unsafe { self.pte_ref(virt, level as u32, frame) };
            *node = Virt::from_ref(pte);

            if pte.present() && !pte.flags().contains(PageTableFlags::HUGE_PAGE) {
                frame = pte
                    .frame()
                    .expect("PageTableEntry should point to a frame if PRESENT bit is set");
>>>>>>> 67b166a (Drop_Subtree)
            } else {
                break;
            }
        }
<<<<<<< HEAD
        let path = Path::new(self, nodes, virt);
        path
    }


    /// Возвращает итератор по листьям дерева отображения страниц,
    /// попадающим в заданный диапазон `range`.
    fn iter_range_mut<R: RangeBounds<Page>>(&mut self, range: R) -> MappingIterator {
        let curr = match range.start_bound() {
            Bound::Included(start) => start.index(),
            Bound::Excluded(start) => start.advance_index(1),
            Bound::Unbounded => Page::lower_half_start_index(),
        };

        let end = match range.end_bound() {
            Bound::Included(end) => end.index() + 1,
            Bound::Excluded(end) => end.index(),
            Bound::Unbounded => Page::higher_half_end_index(),
        };

        MappingIterator {
            _marker: PhantomData,
            curr,
            end,
            mapping: self.into(),
=======

        Path {
            mapping: self,
            nodes,
            virt,
>>>>>>> 67b166a (Drop_Subtree)
        }
    }


    /// Возвращает итератор по листьям дерева отображения страниц.
    pub fn iter_mut(&mut self) -> MappingIterator {
        self.iter_range_mut(..)
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
<<<<<<< HEAD
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
            if level == PAGE_TABLE_LEAF_LEVEL {
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
=======
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
>>>>>>> 67b166a (Drop_Subtree)
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
<<<<<<< HEAD
        debug!(
            "Drop sub_tree start: {:?} {:?} {:?}",
            node, level, drop_used
        );

        let mut all_entries_empty = true;
        let mut page_table: [ku::memory::mmu::PageTableEntry; 512] =
            unsafe { *self.page_table_mut(node) };
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
                        let subtree_empty = { self.drop_subtree(sub_frame, level - 1, drop_used) };
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
=======
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
>>>>>>> 67b166a (Drop_Subtree)
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


    /// Удаляет все рекурсивные записи.
    pub(super) fn remove_recursive_mappings(&mut self) {
        let page_table_root_frame = self.page_table_root;
        let page_table_root = unsafe { self.page_table_mut(page_table_root_frame) };
        for pte in page_table_root.iter_mut() {
            if pte.frame() == Ok(page_table_root_frame) {
                debug!(?pte, "remove recursive mapping");
                pte.clear();
            }
        }
    }


    /// Удаляет ненужную часть --- находящуюся за пределами [`Mapping::phys2virt_mapping`] ---
    /// линейного отображения физической памяти, которое сделал [`bootloader`].
    pub(super) fn remove_excessive_phys2virt(&mut self) -> Result<()> {
        let last_page = Page::from_index(self.phys2virt_mapping.last_page_index().ok_or(NoPage)?)?;
        let mut cleared_pte_count = 0;

        for mut path in self.iter_range_mut(last_page..).skip(1) {
            let (_, pte) = path.deepest_pte_mut();

            if pte.flags().contains(PageTableFlags::PRESENT | PageTableFlags::HUGE_PAGE) {
                pte.clear();
                cleared_pte_count += 1;
            } else {
                break;
            }
        }

        let start_free_frame_count = FRAME_ALLOCATOR.lock().count();

        self.drop_subtree(self.page_table_root, PAGE_TABLE_ROOT_LEVEL, false);

        let freed_frame_count = FRAME_ALLOCATOR.lock().count() - start_free_frame_count;

        debug!(
            freed_frame_count,
            cleared_pte_count,
            "removed excessive phys2virt{}",
            if freed_frame_count == 0 {
                ", but the frames it used are still used or reserved"
            } else {
                ""
            },
        );

        Ok(())
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
        let page_table = self.phys2virt().map(frame.address()).expect("bad frame");
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
<<<<<<< HEAD
        debug!(
            "Starting Mapping::map_intermediate with PageTableEntry: {:?}",
            pte
        );
        let mut alloc = FRAME_ALLOCATOR.lock();
        let frame = alloc.allocate()?;

        unsafe { self.page_table_mut(frame).fill(PageTableEntry::default()) };

        pte.set_frame(frame, pte.flags());

        debug!("Allocation done: {:?}", pte);
=======
        let frame = FRAME_ALLOCATOR.lock().allocate()?;

        unsafe { self.page_table_mut(frame).fill(PageTableEntry::default()) };

        pte.set_frame(frame, PageTableFlags::empty());
>>>>>>> 67b166a (Drop_Subtree)

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
<<<<<<< HEAD
        debug!("Starting map_intermediate with flags: {:?}", flags);
        while let Some((level, node)) = nodes_iter.next() {
            let pte: &mut PageTableEntry = unsafe { node.try_into_mut().unwrap() };
            debug!("Processing node at level {}: {:?}", level, pte);
            if pte.flags().contains(PageTableFlags::HUGE_PAGE) {
                error!("Encountered HUGE_PAGE at level {}", level);
                return Err(Unimplemented);
            }

            if !pte.present() && level != 0 {
                debug!(
                    "Node is not  present. Calling map_intermediate :: mapping {}",
                    level
                );
                self.mapping.map_intermediate(pte)?;
            }

            if let Some((next_level, next_node)) = nodes_iter.peek_mut() {
                debug!("Peeking next node at level {}", next_level);
                let next_pte = unsafe {
                    self.mapping.pte_ref(
                        self.virt,
                        *next_level as u32,
                        pte.frame().expect(
                            "Mapping::map_intermediate should have allocated and set frame",
                        ),
                    )
                };
                debug!("Next PTE retrieved: {:?}", next_pte);
                **next_node = Virt::from_ref(next_pte);
=======

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
>>>>>>> 67b166a (Drop_Subtree)
            }

            pte.set_flags(
                if level != 0 {
<<<<<<< HEAD
                    debug!("Setting flags for non-zero level");
                    pte.flags() | (flags & FULL_ACCESS)
                } else {
                    debug!("Setting flags for level 0");
=======
                    (pte.flags() | flags) & FULL_ACCESS
                } else {
>>>>>>> 67b166a (Drop_Subtree)
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

        if Path::is_leaf(level, pte) {
            let flags = pte.flags();
            let frame = pte.frame().expect(Self::INVALID_PATH);

            let pages = self.pages(level);
            let frames = Block::from_index(frame.index(), frame.index() + pages.count())
                .expect(Self::INVALID_PATH);

            Some(MappedBlock::new(flags, frames, pages))
        } else {
            None
        }
    }


    /// Удаляет отображение страницы по текущему пути.
    /// Физический фрейм освобождается, если на него не осталось других ссылок.
    ///
    /// Возвращает ошибки:
    ///   - [`Error::NoPage`] если промежуточного или нужного листьевого узла таблицы страниц нет.
    ///   - [`Error::Unimplemented`] если промежуточный узел таблицы страниц
    ///     имеет флаг [`PageTableFlags::HUGE_PAGE`].
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
        let level = self
            .nodes
            .iter()
            .position(|&node| node != Virt::default())
            .expect(Self::INVALID_PATH);

        let pte = unsafe { self.nodes[level].converter().expect(Self::INVALID_PATH) };
        let level = level.try_into().expect("unreasonable PTE level");

        (level, pte)
    }


    /// Возвращает блок страниц, который отображает текущий [`Path`].
    /// Аргумент `level` задаёт уровень самой глубокой PTE,
    /// его можно получить из `Path::deepest_pte()`.
    fn pages(&self, level: u32) -> Block<Page> {
        let page_count = PAGE_TABLE_ENTRY_COUNT.pow(level);

        let end = (Page::containing(self.virt).index() + 1).next_multiple_of(page_count);
        let start =
            (Page::containing(self.virt).index() + 1).next_multiple_of(page_count) - page_count;

        Block::from_index(start, end).expect(Self::INVALID_PATH)
    }


    /// Возвращает `true` если [`Path`],
    /// для которого (`level, pte)` получены из `Path::deepest_pte()`,
    /// отвечает присутствующему в [`Mapping`] листу отображения.
    fn is_leaf(level: u32, pte: &PageTableEntry) -> bool {
        pte.present() && (level == 0 || pte.flags().contains(PageTableFlags::HUGE_PAGE))
    }


    /// Возвращает `true` если путь идёт через рекурсивную запись корневого уровня.
    fn is_recursive_entry(&self) -> bool {
        let page_table_root_frame = self.mapping.page_table_root;
        let root_pte = unsafe {
            self.nodes[size::from(PAGE_TABLE_ROOT_LEVEL)]
                .try_into_ref::<PageTableEntry>()
                .expect(Path::INVALID_PATH)
        };

        root_pte.frame() == Ok(page_table_root_frame)
    }


    /// Сообщение паники при некорректно сформированном [`Path`].
    const INVALID_PATH: &'static str = "invalid path";
}


impl<'a> fmt::Display for Path<'a> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let mut frame = self.mapping.page_table_root;
        let mut pte = Err(InvalidArgument);

        for level in (PAGE_TABLE_LEAF_LEVEL..=PAGE_TABLE_ROOT_LEVEL).rev() {
            write!(formatter, "L{}: {}", level, self.nodes[size::from(level)])?;

            pte = unsafe { self.nodes[size::from(level)].try_into_ref::<PageTableEntry>() };

            if let Ok(pte) = pte {
                write!(formatter, " / {}", frame)?;

                if let Ok(next_frame) = pte.frame() {
                    frame = next_frame;
                    if pte.flags().contains(PageTableFlags::HUGE_PAGE) {
                        write!(formatter, " (huge)")?;
                        break;
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

    /// Текущий номер виртуальной страницы, задающий позицию итератора.
    /// Значение, больше либо равное [`MappingIterator::end`], означает, что достигнут конец.
    curr: usize,

    /// Номер виртуальной страницы, задающий позицию за концом диапазона, который пробегает итератор.
    /// Значение, равное [`Page::higher_half_end_index()`], означает, что достигнут конец.
    /// Значения, больше [`Page::higher_half_end_index()`] не допустимы.
    end: usize,

    /// Дерево отображения.
    mapping: NonNull<Mapping>,
}


impl<'a> Iterator for MappingIterator<'a> {
    type Item = Path<'a>;


    fn next(&mut self) -> Option<Self::Item> {
        assert!(self.end <= Page::higher_half_end_index());

        loop {
            if self.curr >= self.end {
                return None;
            }

            let mapping = unsafe { &mut (*self.mapping.as_ptr()) };
            let page = Page::from_index(self.curr)
                .expect("incorrect initialization or advancement of MappingIterator");
            let path = mapping.path(page.address());

            if path.is_recursive_entry() {
                let pages = path.pages(PAGE_TABLE_ROOT_LEVEL);
                self.curr = pages.start_element().advance_index(pages.count());
            } else {
                let (level, pte) = path.deepest_pte();
                let pages = path.pages(level);

                self.curr = pages.start_element().advance_index(pages.count());

                if Path::is_leaf(level, pte) {
                    return Some(path);
                }
            }
        }
    }
}


#[doc(hidden)]
pub(super) mod test_scaffolding {
    use super::super::{
        mmu::{PageTable, PageTableEntry, PAGE_TABLE_LEVEL_COUNT},
        Phys2Virt,
        Virt,
    };

    pub use super::{Mapping, Path};


    pub fn deepest_pte<'a>(path: &'a Path<'a>) -> (u32, &'a PageTableEntry) {
        path.deepest_pte()
    }


    pub fn nodes(path: &Path) -> [Virt; PAGE_TABLE_LEVEL_COUNT] {
        path.nodes
    }


    pub fn page_table_root(mapping: &Mapping) -> &PageTable {
        unsafe { mapping.page_table_ref(mapping.page_table_root()) }
    }


    pub fn phys2virt(mapping: &Mapping) -> Phys2Virt {
        mapping.phys2virt()
    }
}
