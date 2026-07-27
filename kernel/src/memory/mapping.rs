use duplicate::duplicate_item;

use crate::{
    error::{
        Error::{NoPage, Unimplemented},
        Result,
    },
    log::trace,
};

use super::{
    frage::{Frame, Page},
    mmu,
    mmu::{
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
    Virt,
    FRAME_ALLOCATOR,
};

// Used in docs.
#[allow(unused)]
use crate::error::Error;


/// Многоуровневая таблица страниц.
///
/// Фактически дерево большой арности, если игнорировать рекурсивные записи.
#[derive(Debug, Default)]
pub struct Mapping {
    /// Фрейм с корневым узлом таблицы страниц.
    page_table_root: Frame,

    /// Первая страница замапленной полностью физической памяти.
    ///
    /// Через область, которая с неё начинается можно работать со любым участком
    /// физической памяти не отображая его предварительно в виртуальное адресное пространство.
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


    /// Принимает на вход виртуальный адрес `virt`, который нужно транслировать.
    ///
    /// Возвращает отображённый в память префикс пути в дереве трансляции,
    /// соответствующий входному виртуальному адресу `virt`.
    pub fn path(&mut self, virt: Virt) -> Path {
        // TODO: your code here.
        unimplemented!();
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
        // TODO: your code here.
        unimplemented!();
    }


    /// Шаг рекурсии при спуске по дереву отображения страниц.
    /// Выполняет основную работу по освобождению физических фреймов
    /// как отображённых [`Mapping`], так и занятых самим отображением.
    ///
    /// - `node` --- физический фрейм с текущим узлом;
    /// - `level` --- уровень текущего узла в дереве отображения страниц;
    /// - `drop_used` --- равен `true`, если нужно удалить все узлы дерева,
    ///   и `false`, если нужно удалить только узлы, которые фактически не нужны ---
    ///   то есть те, через которые не ведут пути для отображённых страниц.
    ///
    /// Используется в [`Mapping::drop()`] и [`Mapping::unmap_unused_intermediate()`].
    fn drop_subtree(&mut self, node: Frame, level: u32, drop_used: bool) -> bool {
        // TODO: your code here.
        unimplemented!();
    }


    /// Освобождает не использующиеся промежуточные узлы отображения страниц.
    pub fn unmap_unused_intermediate(&mut self) {
        if self.is_valid() {
            if self.drop_subtree(self.page_table_root, PAGE_TABLE_ROOT_LEVEL, false) {
                self.page_table_root = Frame::default();
            }
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


    /// Возвращает мутабельную ссылку на одну запись узла таблицы страниц,
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
    unsafe fn pte_mut(
        &mut self,
        virt: Virt,
        level: u32,
        page_table_frame: Frame,
    ) -> &mut PageTableEntry {
        let index_shift = PAGE_OFFSET_BITS + level * PAGE_TABLE_INDEX_BITS;
        let index = (virt.into_usize() >> index_shift) & PAGE_TABLE_INDEX_MASK;
        let page_table = unsafe { self.page_table_mut(page_table_frame) };

        &mut page_table[index]
    }


    /// Принимает промежуточную запись [`PageTableEntry`],
    /// ссылающуюся на отсутствующий узел дерева отображения.
    /// Выделяет с помощью [`static@FRAME_ALLOCATOR`] фрейм и записывает его
    /// в эту [`PageTableEntry`] вместе с флагом [`PageTableFlags::PRESENT`].
    ///
    /// См. [`Path::map_intermediate()`] в котором этот метод
    /// используется как вспомогательный.
    ///
    /// # Safety
    ///
    /// Вызывающий код должен гарантировать,
    /// что инварианты управления памятью в Rust'е не будут нарушены.
    /// В частности, нет других ссылок, которые ведут в ту же запись [`PageTableEntry`].
    fn map_intermediate(&mut self, pte: &mut PageTableEntry) -> Result<()> {
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
        // TODO: your code here.
        unimplemented!();
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
