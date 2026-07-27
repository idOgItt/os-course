use core::{arch::asm, fmt, mem};

use bitflags::bitflags;
use static_assertions::const_assert;

use crate::{
    error::{Error::NoPage, Result},
    log::warn,
};

use super::{
    addr::Phys,
    frage::{Frame, Page},
};


/// Запись таблицы страниц.
///
/// Аналогична [`x86_64::structures::paging::page_table::PageTableEntry`].
#[derive(Clone, Copy, Default, Eq, PartialEq)]
#[repr(transparent)]
pub struct PageTableEntry(usize);


impl PageTableEntry {
    /// Маска адреса физического фрейма, на который указывает эта запись.
    const ADDRESS_MASK: usize =
        ((1 << ((PAGE_TABLE_ROOT_LEVEL + 1) * PAGE_TABLE_INDEX_BITS)) - 1) << PAGE_OFFSET_BITS;


    /// Физический фрейм, на который указывает эта запись.
    pub fn frame(&self) -> Result<Frame> {
        if self.present() {
            Ok(Frame::new(self.phys()).unwrap())
        } else {
            Err(NoPage)
        }
    }


    /// Устанавливает целевой физического фрейм `frame` и
    /// флаги доступа `flags` в данной записи.
    /// Принудительно выставляет флаг [`PageTableFlags::PRESENT`].
    pub fn set_frame(&mut self, frame: Frame, flags: PageTableFlags) {
        self.set_phys(frame.address(), flags | PageTableFlags::PRESENT);
    }


    /// Флаги доступа к странице, которую описывает эта запись.
    pub fn flags(&self) -> PageTableFlags {
        PageTableFlags::from_bits(self.0 & !Self::ADDRESS_MASK).unwrap()
    }


    /// Устанавливает флаги доступа к странице, которую описывает эта запись.
    pub fn set_flags(&mut self, flags: PageTableFlags) {
        self.set_phys(self.phys(), flags);
    }


    /// Возвращает `true`, если запись используется.
    pub fn present(&self) -> bool {
        self.flags().contains(PageTableFlags::PRESENT)
    }


    /// Очищает запись.
    pub fn clear(&mut self) {
        self.0 = 0;
    }


    /// Адреса физического фрейма, на который указывает эта запись.
    fn phys(&self) -> Phys {
        Phys::new(self.0 & Self::ADDRESS_MASK).unwrap()
    }


    /// Устанавливает адрес физического фрейма `phys` и флаги доступа `flags`.
    fn set_phys(&mut self, phys: Phys, flags: PageTableFlags) {
        self.0 = phys.into_usize() | flags.bits();
    }
}


impl fmt::Debug for PageTableEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        if let Ok(frame) = self.frame() {
            let flags = self.flags();
            write!(formatter, "{} {}", frame, flags)
        } else {
            write!(formatter, "<non-present>")
        }
    }
}


/// Читает из регистра `CR3` физический адрес корневого узла текущей таблицы страниц.
pub fn page_table_root() -> Frame {
    let page_table_root: usize;

    unsafe {
        asm!(
            "mov {page_table_root}, cr3",
            page_table_root = out(reg) page_table_root,
        );
    }

    let flags = PageTableFlags::from_bits_truncate(page_table_root);
    let page_table_root = Frame::containing(Phys::new(page_table_root).unwrap());

    if !flags.is_empty() {
        warn!(%page_table_root, ?flags, "non empty flags for the page directory are wrong");
    }

    page_table_root
}


/// Записывает в регистра `CR3` физический адрес корневого узла таблицы страниц.
/// При этом процессор переключается в виртуальное адресное пространство,
/// задаваемое этой таблицей страниц.
///
/// # Safety
///
/// Вызывающий код должен гарантировать сохранение инвариантов работы с памятью в Rust.
/// В частности, что не осталось ссылок, ведущих в страницы, для которых целевое
/// страничное отображение отличается от текущего.
pub unsafe fn set_page_table_root(frame: Frame) {
    unsafe {
        asm!(
            "mov cr3, {pte}",
            pte = in(reg) frame.address().into_usize(),
        );
    }
}


/// Узел таблицы страниц.
/// Аналогичен [`x86_64::structures::paging::page_table::PageTable`].
pub type PageTable = [PageTableEntry; PAGE_TABLE_ENTRY_COUNT];


bitflags! {
    /// Флаги в записи таблицы страниц.
    /// Аналогичны [`x86_64::structures::paging::page_table::PageTableFlags`].
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct PageTableFlags: usize {
        /// Страница отображена в память.
        const PRESENT = 1 << 0;

        /// Страница доступна на запись.
        const WRITABLE = 1 << 1;

        /// Страница доступна в режиме пользователя.
        const USER_ACCESSIBLE = 1 << 2;

        /// Для страницы используется синхронная запись в память, без ожидания буфера записи.
        const WRITE_THROUGH = 1 << 3;

        /// Кеширование страницы запрещено.
        const NO_CACHE = 1 << 4;

        /// К странице был доступ.
        const ACCESSED = 1 << 5;

        /// В страницу была запись.
        const DIRTY = 1 << 6;

        /// Большая страница вместо следующего уровня таблицы страниц.
        const HUGE_PAGE = 1 << 7;

        /// Страницу не нужно сбрасывать при сбросе всего TLB.
        const GLOBAL = 1 << 8;

        /// Биты, доступные ОС для произвольных нужд.
        const AVAILABLE = 0b111 << 9;

        /// Один из битов [`PageTableFlags::AVAILABLE`] используется
        /// для пометки страниц, которые должны быть скопированы в случае записи в них.
        const COPY_ON_WRITE = 1 << 9;

        /// Запрет интерпретировать данные в странице как код, требует включения флага [`x86_64::registers::model_specific::EferFlags::NO_EXECUTE_ENABLE`] в [`x86_64::registers::model_specific::Efer`].
        const NO_EXECUTE = 1 << 63;
    }
}


impl fmt::Display for PageTableFlags {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        /// Сокращения для флагов отображения страниц.
        static FLAGS: [char; 9] = ['P', 'W', 'U', 'T', 'C', 'A', 'D', 'H', 'G'];

        write!(
            formatter,
            "{:X}",
            (*self & PageTableFlags::AVAILABLE).bits() >> FLAGS.len(),
        )?;

        for (bit, flag) in FLAGS.iter().enumerate().rev() {
            write!(
                formatter,
                "{}",
                if self.bits() & (1 << bit) != 0 {
                    *flag
                } else {
                    '-'
                },
            )?;
        }

        write!(
            formatter,
            "{}({:#03X})",
            if self.contains(PageTableFlags::NO_EXECUTE) {
                '-'
            } else {
                'X'
            },
            self.bits(),
        )
    }
}


/// Флаги для страниц, доступных ядру только на чтение.
pub const KERNEL_READ: PageTableFlags =
    PageTableFlags::from_bits_truncate(PageTableFlags::PRESENT.bits());

/// Флаги для страниц, доступных ядру на чтение и запись.
pub const KERNEL_RW: PageTableFlags =
    PageTableFlags::from_bits_truncate(KERNEL_READ.bits() | PageTableFlags::WRITABLE.bits());

/// Шаблон флагов для страниц, доступных коду пользователя.
pub const USER: PageTableFlags = PageTableFlags::from_bits_truncate(
    PageTableFlags::PRESENT.bits() | PageTableFlags::USER_ACCESSIBLE.bits(),
);

/// Флаги для страниц, доступных коду пользователя только на чтение.
pub const USER_READ: PageTableFlags =
    PageTableFlags::from_bits_truncate(KERNEL_READ.bits() | PageTableFlags::USER_ACCESSIBLE.bits());

/// Флаги для страниц, доступных коду пользователя на чтение и запись.
pub const USER_RW: PageTableFlags =
    PageTableFlags::from_bits_truncate(KERNEL_RW.bits() | PageTableFlags::USER_ACCESSIBLE.bits());

/// Шаблон флагов для страниц, не ограничивающий доступ.
pub const FULL_ACCESS: PageTableFlags = PageTableFlags::from_bits_truncate(
    PageTableFlags::PRESENT.bits() |
        PageTableFlags::WRITABLE.bits() |
        PageTableFlags::USER_ACCESSIBLE.bits(),
);

/// Шаблон флагов для страниц, который пользователь может задавать в системных вызовах.
pub const SYSCALL_ALLOWED_FLAGS: PageTableFlags =
    PageTableFlags::from_bits_truncate(USER_RW.bits() | PageTableFlags::COPY_ON_WRITE.bits());


/// Количество бит в смещении внутри виртуальной страницы или физического фрейма.
/// Равно двоичному логарифму их размера.
pub const PAGE_OFFSET_BITS: u32 = 12;

/// Количество записей в одном узле таблицы страниц.
pub const PAGE_TABLE_ENTRY_COUNT: usize = 1 << PAGE_TABLE_INDEX_BITS;

/// Количество бит в номере записи внутри одного узла таблицы страниц.
pub const PAGE_TABLE_INDEX_BITS: u32 = 9;

/// Маска для вырезания из адреса индекса в каком-нибудь из узлов таблицы страниц.
pub const PAGE_TABLE_INDEX_MASK: usize = (1 << PAGE_TABLE_INDEX_BITS) - 1;

/// Номер листьевого уровня таблицы страниц.
pub const PAGE_TABLE_LEAF_LEVEL: u32 = 0;

/// Номер корневого уровня таблицы страниц.
pub const PAGE_TABLE_ROOT_LEVEL: u32 = 3;

/// Количество уровней в дереве отображения.
pub const PAGE_TABLE_LEVEL_COUNT: usize = PAGE_TABLE_ROOT_LEVEL as usize + 1;


const_assert!(
    mem::size_of::<PageTable>() == PAGE_TABLE_ENTRY_COUNT * mem::size_of::<PageTableEntry>()
);
const_assert!(mem::size_of::<PageTable>() == Page::SIZE);
