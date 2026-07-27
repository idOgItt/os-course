use core::{fmt, mem::MaybeUninit, num::NonZeroUsize};

use itertools::Itertools;

use ku::{
    error::{
        Error::{InvalidArgument, NoPage, Overflow, PermissionDenied},
        Result,
    },
    memory::{
        mmu::{
            PageTable,
            PageTableEntry,
            PageTableFlags,
            KERNEL_RW,
            PAGE_OFFSET_BITS,
            PAGE_TABLE_ENTRY_COUNT,
            PAGE_TABLE_INDEX_BITS,
            PAGE_TABLE_LEAF_LEVEL,
            PAGE_TABLE_ROOT_LEVEL,
        },
        Block,
        Frame,
        Page,
        Phys,
        Size,
        Virt,
    },
};

use crate::memory::FRAME_ALLOCATOR;

// Used in docs.
#[allow(unused)]
use crate::error::Error;


/// Для простоты работы с физической памятью,
/// она целиком линейно отображена в некоторую область виртуальной.
/// [`Phys2Virt`] описывает это отображение.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Phys2Virt {
    /// Блок виртуальной памяти, куда линейно отображена вся физическая память.
    mapping: Block<Page>,

    /// Блок, описывающий всю доступную физическую память.
    physical_memory: Block<Frame>,

    /// Количество использованных для отображения физических фреймов.
    /// Или [`None`], если оно не известно, так как отображение построил [`bootloader`].
    used_frame_count: Option<NonZeroUsize>,
}


impl Phys2Virt {
    /// Создаёт [`Phys2Virt`] по его начальной странице `start` и
    /// блоку всей физической памяти `physical_memory`.
    pub(super) fn new(physical_memory: Block<Frame>, start: Page) -> Result<Self> {
        Self::new_impl(physical_memory, start, None)
    }


    /// Возвращает пустое отображение [`Phys2Virt`].
    /// В отличие от [`Phys2Virt::default()`] доступна в константном контексте.
    pub(super) const fn zero() -> Self {
        Self {
            mapping: Block::zero(),
            physical_memory: Block::zero(),
            used_frame_count: None,
        }
    }


    /// Возвращает индекс первой страницы за концом[`Phys2Virt`]
    /// или [`None`] для пустого [`Phys2Virt`].
    pub(super) fn last_page_index(&self) -> Option<usize> {
        if self.mapping.is_empty() {
            None
        } else {
            self.mapping.end().checked_sub(1)
        }
    }


    /// Возвращает первую страницу [`Phys2Virt`].
    pub(super) fn start(&self) -> Page {
        Page::containing(self.mapping.start_address())
    }


    /// Для заданного физического адреса `phys` возвращает виртуальный адрес
    /// внутри специальной области [`Phys2Virt`],
    /// в которую линейно отображена вся физическая память.
    ///
    /// Возвращает ошибку [`Error::Overflow`] если адрес `phys` не попадает в
    /// физическую память, имеющуюся в системе.
    pub fn map(&self, phys: Phys) -> Result<Virt> {
        let virt = (self.mapping.start_address() + phys.into_usize())?;
        if self.mapping.contains_address(virt) {
            Ok(virt)
        } else {
            Err(Overflow)
        }
    }


    // ANCHOR: make
    /// Создаёт отображение [`Phys2Virt`],
    /// используя рекурсивную запись `recursive_mapping`.
    ///
    /// Для экономии физических фреймов под  [`Phys2Virt`]:
    ///   - Использует страницы размером по 1 GiB.
    ///   - Учитывает, что фреймы вне [`Phys2Virt::physical_memory`]
    ///     никогда не будут переданы в [`Phys2Virt::map()`].
    ///     Так как их нет в физической памяти.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidArgument`] --- номер рекурсивной записи `recursive_mapping`
    ///   выходит за пределы узла [`PageTable`] дерева отображения страниц.
    ///   Или равен нулю, чего не может быть так как под нулевой записью верхнего уровня есть
    ///   зарезервированные участки памяти.
    /// - [`Error::NoFrame`] --- не хватило свободной физической памяти.
    /// - [`Error::NoPage`] --- в корневом узле дерева отображения виртуальных страниц
    ///   не нашлось достаточного количества записей для отображения блока `physical_memory`.
    /// - [`Error::PermissionDenied`] --- рекурсивная запись `recursive_mapping` попадает в
    ///   пользовательскую часть виртуальной памяти.
    pub(super) fn make(physical_memory: Block<Frame>, recursive_mapping: usize) -> Result<Self> {
        // ANCHOR_END: make
        if recursive_mapping == 0 || recursive_mapping >= PAGE_TABLE_ENTRY_COUNT {
            return Err(InvalidArgument);
        }
        if super::user_root_level_entries().contains(&recursive_mapping) {
            return Err(PermissionDenied);
        }

        // TODO: your code here.
        unimplemented!();
    }


    /// Находит во второй половине корневого узла страниц. То есть, в верхней
    /// [половине адресного пространства](https://en.wikipedia.org/wiki/X86-64#Virtual_address_space_details) ---
    /// последовательность из `root_entry_count` не используемых записей.
    /// Возвращает номер первой из этих записей.
    ///
    /// Использует рекурсивную запись номер `recursive_mapping`.
    ///
    /// # Errors
    ///
    /// - [`Error::NoPage`] --- во второй половине корневого узле дерева отображения
    ///   виртуальных страниц не нашлось `root_entry_count` свободных записей.
    fn find_empty_root_entries(root_entry_count: usize, recursive_mapping: usize) -> Result<usize> {
        // TODO: your code here.
        unimplemented!();
    }


    /// Инициализирует записи 1 GiB страниц для отображения [`Phys2Virt`].
    ///
    /// Начинает с корневой записи `start_root_entry` и
    /// инициализирует `root_entry_count` корневых записей.
    /// А также инициализирует `entry_count` записей следующего уровня,
    /// каждая из которых отвечает за 1 GiB виртуальной памяти.
    ///
    /// Использует рекурсивную запись номер `recursive_mapping`.
    ///
    /// # Errors
    ///
    /// - [`Error::NoFrame`] --- не хватило свободной физической памяти.
    fn fill_entries(
        start_root_entry: usize,
        root_entry_count: usize,
        entry_count: usize,
        recursive_mapping: usize,
    ) -> Result<()> {
        // TODO: your code here.
        unimplemented!();
    }


    /// Выделяет фрейм для дочернего узла номер `root_entry` корневого узла таблицы страниц,
    /// инициализирует его пустыми записями ---- [`PageTableEntry::default()`] --- и
    /// записывает его в соответствующую [`PageTableEntry`].
    ///
    /// Использует рекурсивную запись номер `recursive_mapping`.
    ///
    /// # Errors
    ///
    /// - [`Error::NoFrame`] --- не хватило свободной физической памяти.
    /// - [`Error::NoPage`] --- корневая запись номер `root_entry` уже занята.
    fn allocate_root_entry(root_entry: usize, recursive_mapping: usize) -> Result<()> {
        // TODO: your code here.
        unimplemented!();
    }


    /// Возвращает узел таблицы страниц,
    /// на который ссылается запись номер `index` корневого узла таблицы страниц.
    ///   - При `index == recursive_mapping` этот узел --- сам корневой.
    ///   - При `index != recursive_mapping` это узел следующего уровня,
    ///     каждая запись в нём отвечает за 1 GiB адресного пространства.
    ///
    /// Возвращает блок размером в одну 4 KiB страницу,
    /// который удобно преобразовывать с помощью `Block::try_into*()`
    /// в подходящий тип для работы с узлом дерева отображения.
    ///
    /// Использует рекурсивную запись номер `recursive_mapping`.
    fn node(index: usize, recursive_mapping: usize) -> Block<Page> {
        // TODO: your code here.
        unimplemented!();
    }


    /// Создаёт [`Phys2Virt`] по его начальной странице `start` и
    /// блоку всей физической памяти `physical_memory`.
    ///
    /// Сохраняет `used_frame_count` --- количество физических фреймов,
    /// которые были потрачены на создание отображения `Phys2Virt`.
    /// Или [`None`], если это количество не известно --- отображение создано [`bootloader`].
    fn new_impl(
        physical_memory: Block<Frame>,
        start: Page,
        used_frame_count: Option<NonZeroUsize>,
    ) -> Result<Self> {
        Ok(Self {
            mapping: Block::from_index(start.index(), start.index() + physical_memory.count())?,
            physical_memory,
            used_frame_count,
        })
    }


    /// Номер уровня дерева отображения страниц, на котором записи отвечают за
    /// самые большие поддерживаемые страницы виртуальной памяти (по факту за 1 GiB страницы).
    #[allow(non_upper_case_globals)]
    const LEVEL_FOR_1_GiB_ENTRIES: u32 = PAGE_TABLE_ROOT_LEVEL - 1;

    /// Количество обычных 4 KiB страниц, помещающихся в одной виртуальной странице
    /// самого большого поддерживаемого размера (по факту 1 GiB / 4 KiB).
    #[allow(non_upper_case_globals)]
    const PAGES_PER_1_GiB_ENTRY: usize = PAGE_TABLE_ENTRY_COUNT.pow(Self::LEVEL_FOR_1_GiB_ENTRIES);
}


impl fmt::Display for Phys2Virt {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{} -> {}", self.mapping, self.physical_memory)?;

        if let Some(used_frame_count) = self.used_frame_count {
            write!(
                formatter,
                "; uses {} for the mapping itself",
                Size::new::<Frame>(used_frame_count.get()),
            )?;
        }

        Ok(())
    }
}


#[doc(hidden)]
pub(super) mod test_scaffolding {
    use ku::error::Result;

    use super::{
        super::{Block, Frame},
        Phys2Virt,
    };


    pub fn make_phys2virt(
        physical_memory: Block<Frame>,
        recursive_mapping: usize,
    ) -> Result<Phys2Virt> {
        Phys2Virt::make(physical_memory, recursive_mapping)
    }


    pub(in super::super) fn physical_memory(phys2virt: &Phys2Virt) -> Block<Frame> {
        phys2virt.physical_memory
    }
}
