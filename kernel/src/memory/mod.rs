/// Дополнительные функции для работы с [Memory Management Unit](https://en.wikipedia.org/wiki/Memory_management_unit), доступные только ядру.
pub mod mmu;

/// Глобальная таблица дескрипторов [`Gdt`]
/// ([Global Descriptor Table](https://en.wikipedia.org/wiki/Global_Descriptor_Table), GDT).
pub(crate) mod gdt;

/// Абстракция адресного пространства [`AddressSpace`].
mod address_space;

/// Временный аллокатор физических фреймов [`BootFrameAllocator`].
mod boot_frame_allocator;

/// Основной аллокатор физических фреймов [`MainFrameAllocator`].
mod main_frame_allocator;

/// Отображённый блок адресного пространства [`MappedBlock`].
mod mapped_block;

/// Реализация отображения виртуальной памяти в физическую [`Mapping`].
mod mapping;

/// Аллокатор страниц виртуальной памяти [`PageAllocator`].
mod page_allocator;

/// Для простоты работы с физической памятью,
/// она целиком замаплена в некоторую область виртуальной.
/// [`Phys2Virt`] описывает это отображение.
mod phys2virt;

/// Работа со стеками [`Stack`] и
/// выделенные стеки для непредвиденных исключений [`ExceptionStacks`].
mod stack;

/// Сегмент состояния задачи
/// ([Task State Segment](https://en.wikipedia.org/wiki/Task_state_segment), TSS).
mod tss;


use core::{cmp, mem, ops::Range};

use bootloader::{
    bootinfo::{MemoryMap, MemoryRegionType},
    BootInfo,
};
use derive_more::Deref;
use lazy_static::lazy_static;
use static_assertions::const_assert;
use x86_64::registers::model_specific::{Efer, EferFlags};

pub use ku::{
    memory::{addr, block, frage, size},
    sync::spinlock::Spinlock,
};

use crate::{
    error::{
        Error::{NoFrame, PermissionDenied},
        Result,
    },
    log::{debug, error, info, warn},
    time,
    Subsystems,
};

use boot_frame_allocator::BootFrameAllocator;
use main_frame_allocator::MainFrameAllocator;
use mapping::Mapping;
use mmu::{PageTableFlags, PAGE_TABLE_ENTRY_COUNT, PAGE_TABLE_ROOT_LEVEL};

pub use addr::{Phys, Virt};
pub use address_space::{AddressSpace, BASE_ADDRESS_SPACE};
pub use block::Block;
pub use frage::{Frame, Page};
pub use mmu::{KERNEL_READ, KERNEL_RW, USER, USER_READ, USER_RW};
pub use size::{Size, SizeOf};

pub(crate) use gdt::{Gdt, RealModePseudoDescriptor, GDT};
pub(crate) use phys2virt::Phys2Virt;
pub(crate) use stack::{Stack, EXCEPTION_STACKS};
pub(crate) use tss::{DOUBLE_FAULT_IST_INDEX, PAGE_FAULT_IST_INDEX};

// Used in docs.
#[allow(unused)]
use {
    crate::{self as kernel, error::Error},
    mapped_block::MappedBlock,
    page_allocator::PageAllocator,
    stack::ExceptionStacks,
};


/// Инициализация подсистемы памяти.
pub(super) fn init(boot_info: &'static BootInfo, subsystems: Subsystems) -> Result<Phys2Virt> {
    let timer = time::timer();

    unsafe {
        Efer::write(EferFlags::NO_EXECUTE_ENABLE | Efer::read());
    }

    let physical_memory = physical_memory(&boot_info.memory_map);
    test_scaffolding::set_frame_count(physical_memory.end());

    if subsystems.contains(Subsystems::PHYS_MEMORY) {
        *FRAME_ALLOCATOR.lock() =
            FrameAllocator::Boot(BootFrameAllocator::new(&boot_info.memory_map));
    }

    let phys2virt = if cfg!(feature = "bootloader-phys2virt") {
        let start = phys2virt(boot_info);
        Phys2Virt::new(physical_memory, start)
    } else {
        let recursive_mapping = size::from(boot_info.recursive_index());
        test_scaffolding::set_recursive_mapping(recursive_mapping);
        Phys2Virt::make(physical_memory, recursive_mapping)
    }?;

    info!(%phys2virt);

    if subsystems.contains(Subsystems::VIRT_MEMORY) {
        let page_table_root = Mapping::current_page_table_root();
        let mut address_space = AddressSpace::new(page_table_root, phys2virt);

        let page_table_root = unsafe { address_space.mapping().page_table_ref(page_table_root) };
        *KERNEL_PAGE_ALLOCATOR.lock() =
            PageAllocator::new(page_table_root, 0..KERNEL_ROOT_LEVEL_ENTRY_COUNT);

        *BASE_ADDRESS_SPACE.lock() = address_space;
    }

    if subsystems.contains(Subsystems::MAIN_FRAME_ALLOCATOR) {
        if let FrameAllocator::Boot(boot_frame_allocator) = &*FRAME_ALLOCATOR.lock() {
            assert!(boot_frame_allocator.is_consistent());
        }

        *FRAME_ALLOCATOR.lock() = FrameAllocator::Main(main_frame_allocator::init(
            physical_memory,
            &boot_info.memory_map,
        ));
    }

    if subsystems.contains(Subsystems::VIRT_MEMORY | Subsystems::MAIN_FRAME_ALLOCATOR) {
        if let Err(error) = EXCEPTION_STACKS.lock().make_guard_zones() {
            error!(
                ?error,
                "failed to make guard zones for the exception stacks",
            );
        }

        BASE_ADDRESS_SPACE.lock().remove_excessive_phys2virt()?;
    }

    info!(duration = %timer.elapsed(), "memory init");

    Ok(phys2virt)
}


/// Возвращает виртуальную страницу, с которой начинается линейное отображение
/// всей физической памяти в виртуальную [`Phys2Virt`].
#[cfg(feature = "bootloader-phys2virt")]
fn phys2virt(boot_info: &BootInfo) -> Page {
    Virt::new_u64(boot_info.physical_memory_offset)
        .and_then(Page::new)
        .expect("invalid phys2virt in BootInfo")
}


/// Возвращает нулевую виртуальную страницу.
#[cfg(not(feature = "bootloader-phys2virt"))]
fn phys2virt(_boot_info: &BootInfo) -> Page {
    Page::default()
}


lazy_static! {
    /// Аллокатор физических фреймов.
    pub static ref FRAME_ALLOCATOR: Spinlock<FrameAllocator> = Spinlock::new(FrameAllocator::Void);
}


/// Аллокатор физических фреймов.
pub enum FrameAllocator {
    /// Пустой аллокатор физических фреймов.
    Void,

    /// Временный аллокатор физических фреймов, используется при инициализации подсистемы памяти.
    Boot(BootFrameAllocator),

    /// Основной аллокатор физических фреймов.
    Main(MainFrameAllocator),
}


#[allow(rustdoc::private_intra_doc_links)]
impl FrameAllocator {
    /// Возвращает количество свободных физических фреймов у аллокатора [`FrameAllocator`].
    pub fn count(&self) -> usize {
        match self {
            FrameAllocator::Void => 0,
            FrameAllocator::Boot(boot_allocator) => boot_allocator.count(),
            FrameAllocator::Main(main_allocator) => main_allocator.count(),
        }
    }


    /// Выделяет ровно один физический фрейм.
    ///
    /// Если свободных физических фреймов не осталось,
    /// возвращает ошибку [`Error::NoFrame`].
    pub fn allocate(&mut self) -> Result<Frame> {
        match self {
            FrameAllocator::Void => Err(NoFrame),
            FrameAllocator::Boot(boot_allocator) => boot_allocator.allocate(),
            FrameAllocator::Main(main_allocator) => main_allocator.allocate(),
        }
    }


    /// Уменьшает на единицу счётчик использований заданного физического фрейма `frame`.
    /// Физический фрейм освобождается, если на него не осталось других ссылок.
    ///
    /// # Panics
    ///
    /// Паникует, если фрейм свободен или
    /// если в данный момент за аллокацию отвечает не [`MainFrameAllocator`].
    pub fn deallocate(&mut self, frame: Frame) {
        match self {
            FrameAllocator::Boot(boot_allocator) => boot_allocator.deallocate(frame),
            FrameAllocator::Main(main_allocator) => main_allocator.deallocate(frame),
            _ => panic!("can not deallocate a frame without a frame allocator"),
        }
    }


    /// Увеличивает на единицу счётчик использований заданного физического фрейма `frame`.
    ///
    /// # Panics
    ///
    /// Паникует, если фрейм свободен или
    /// если в данный момент за аллокацию отвечает не [`MainFrameAllocator`].
    pub fn reference(&mut self, frame: Frame) {
        match self {
            FrameAllocator::Boot(boot_allocator) => boot_allocator.reference(frame),
            FrameAllocator::Main(main_allocator) => main_allocator.reference(frame),
            _ => panic!("can not reference a frame without a frame allocator"),
        }
    }


    /// Возвращает количество ссылок на `frame`.
    ///
    /// - Если `frame` свободен, возвращается `0`.
    /// - Если он отсутствует или зарезервирован --- [`Error::NoFrame`].
    ///
    /// # Panics
    ///
    /// Паникует, если в данный момент за аллокацию отвечает не [`MainFrameAllocator`].
    pub fn reference_count(&self, frame: Frame) -> Result<usize> {
        match self {
            FrameAllocator::Main(main_allocator) => main_allocator.reference_count(frame),
            _ => panic!("can not get the reference count without the main frame allocator"),
        }
    }


    /// Проверяет, что заданный физический фрейм уже был аллоцирован.
    ///
    /// # Panics
    ///
    /// Паникует, если в данный момент ни [`BootFrameAllocator`] ни [`MainFrameAllocator`]
    /// ещё не используются.
    fn is_used(&self, frame: Frame) -> bool {
        match self {
            FrameAllocator::Void => {
                panic!("no frames should be marked as used by void frame allocator")
            },
            FrameAllocator::Boot(boot_allocator) => boot_allocator.is_used(frame),
            FrameAllocator::Main(main_allocator) => main_allocator.is_used(frame),
        }
    }
}


/// RAII для одной аллокации [`Frame`].
///
/// Вызывает [`FrameAllocator::deallocate()`] для [frame][Frame] при
/// [`FrameDeallocatorGuard::drop()`], если [frame][Frame]
/// не был забран ранее с помощью [`FrameDeallocatorGuard::take()`].
///
/// # Examples
///
/// ## Автоматическое освобождениe фрейма в случае ошибки
/// ```rust
/// # fn allocate_frame() -> Frame {
/// #     Frame::zero()
/// # }
/// #
/// # fn fallible_initialization(_frame: Frame) -> Result<()> {
/// #     Ok(())
/// # }
/// #
/// fn get_initialized_frame() -> Result<Frame> {
///     let frame = FrameDeallocatorGuard::new(allocate_frame());
///
///     // Благодаря тому, что FrameDeallocatorGuard реализует типаж Deref<Target = Frame>,
///     // принадлежащий ему Frame доступен через разыменование.
///     // В случае, если этот вызов возвращает Err(...), утечки не происходит и Frame освобождается
///     // автоматически.
///     fallible_initialization(*frame)?;
///     Ok(frame.take())
/// }
/// ```
#[derive(Deref)]
pub struct FrameDeallocatorGuard {
    /// Фрейм, защищаемый от утечки.
    frame: Frame,
}


impl FrameDeallocatorGuard {
    /// Создает новый [`FrameDeallocatorGuard`] для данного [`frame`][Frame].
    pub fn new(frame: Frame) -> Self {
        Self { frame }
    }


    /// Создает новый [`FrameDeallocatorGuard`] от выражения
    /// [`FRAME_ALLOCATOR.lock().allocate()`][FrameAllocator::allocate].
    ///
    /// Если свободных физических фреймов не осталось,
    /// возвращает ошибку [`Error::NoFrame`].
    pub fn allocate() -> Result<Self> {
        Ok(Self::new(FRAME_ALLOCATOR.lock().allocate()?))
    }


    /// Забирает [`Frame`] из данного [`FrameDeallocatorGuard`].
    pub fn take(self) -> Frame {
        let frame = self.frame;
        mem::forget(self);
        frame
    }
}


impl Drop for FrameDeallocatorGuard {
    fn drop(&mut self) {
        FRAME_ALLOCATOR.lock().deallocate(self.frame);
    }
}


/// Для простоты работы с ней, физическая память целиком замаплена в некоторую область виртуальной.
/// Эта функция по адресу первой (виртуальной) страницы этой области `phys2virt`
/// выдаёт соответствующий виртуальный адрес для заданного физического `address`.
#[allow(unused)]
pub(crate) fn phys2virt_map(phys2virt: Page, address: Phys) -> Virt {
    let message = "bad phys2virt";
    let frame = Frame::containing(address);
    let pseudo_physical_memory =
        Block::new(Frame::default(), (frame + 1).expect(message)).expect(message);

    Phys2Virt::new(pseudo_physical_memory, phys2virt)
        .expect(message)
        .map(address)
        .expect(message)
}


/// Возвращает `true`, если весь блок `pages` зарезервирован для ядра.
pub(crate) fn is_kernel_block(pages: Block<Page>) -> bool {
    !is_user_block(pages) && user_pages().is_disjoint(pages)
}


/// Возвращает `true`, если весь блок `pages` зарезервирован для пространства пользователя.
pub(crate) fn is_user_block(pages: Block<Page>) -> bool {
    user_pages().contains_block(pages)
}


/// Возвращает `true`, если страница `page` зарезервирована для пространства пользователя.
fn is_user_page(page: Page) -> bool {
    user_pages().contains(page)
}


/// Возвращает диапазон записей в корневом узле таблицы страниц,
/// который зарезервирован для пространства ядра.
const fn kernel_root_level_entries() -> Range<usize> {
    0..KERNEL_ROOT_LEVEL_ENTRY_COUNT
}


/// Возвращает диапазон записей в корневом узле таблицы страниц,
/// который зарезервирован для пространства пользователя.
const fn user_root_level_entries() -> Range<usize> {
    KERNEL_ROOT_LEVEL_ENTRY_COUNT..LOWER_HALF_ROOT_LEVEL_ENTRY_COUNT
}


/// Возвращает блок страниц,
/// который зарезервирован для пространства пользователя.
fn user_pages() -> Block<Page> {
    let user_root_level_entries = user_root_level_entries();
    Block::from_index(
        user_root_level_entries.start * PAGES_PER_ROOT_LEVEL_ENTRY,
        user_root_level_entries.end * PAGES_PER_ROOT_LEVEL_ENTRY,
    )
    .expect("virtual memory block reserved for user space is invalid")
}


/// Возвращает ошибку [`Error::PermissionDenied`] если в блоке есть как страницы,
/// которые лежат в зарезервированной для пользователя области,
/// так и страницы, которые лежат в зарезервированной для ядра области.
fn validate_block(pages: Block<Page>) -> Result<()> {
    if is_kernel_block(pages) != is_user_block(pages) {
        Ok(())
    } else {
        Err(PermissionDenied)
    }
}


/// Возвращает ошибку [`Error::PermissionDenied`] если принадлежность пользователю
/// какой-нибудь страницы блока `pages` не соответствует запрошенным флагам.
///
/// То есть, адрес этой страницы лежит в зарезервированной для пользователя области
/// [`kernel::memory::user_pages()`], а во `flags` нет доступа пользователю ---
/// флага [`PageTableFlags::USER_ACCESSIBLE`].
/// Либо наоборот, флаг [`PageTableFlags::USER_ACCESSIBLE`] есть, но страница не
/// лежит в области [`kernel::memory::user_pages()`].
fn validate_block_flags(pages: Block<Page>, flags: PageTableFlags) -> Result<()> {
    validate_block(pages)?;

    if is_user_block(pages) == flags.contains(PageTableFlags::USER_ACCESSIBLE) {
        Ok(())
    } else {
        Err(PermissionDenied)
    }
}


/// Возвращает ошибку [`Error::PermissionDenied`] если принадлежность пользователю
/// страницы `page` не соответствует запрошенным флагам.
///
/// То есть, адрес этой страницы лежит в зарезервированной для пользователя области
/// [`kernel::memory::user_pages()`], а во `flags` нет доступа пользователю ---
/// флага [`PageTableFlags::USER_ACCESSIBLE`].
/// Либо наоборот, флаг [`PageTableFlags::USER_ACCESSIBLE`] есть, но страница не
/// лежит в области [`kernel::memory::user_pages()`].
fn validate_page_flags(page: Page, flags: PageTableFlags) -> Result<()> {
    if is_user_page(page) == flags.contains(PageTableFlags::USER_ACCESSIBLE) {
        Ok(())
    } else {
        Err(PermissionDenied)
    }
}


/// Возвращает блок, описывающий всю доступную физическую память.
/// Гарантированно начинается с нулевого фрейма.
///
/// В этот блок могут не попадать некоторые зарезервированные диапазоны фреймов.
/// Поэтому он может быть немного меньше установленной в системе физической памяти.
fn physical_memory(memory_map: &MemoryMap) -> Block<Frame> {
    let mut prev_physical_region = Block::default();

    let mut total_frame_count = 0;
    let mut usable_frame_count = 0;

    let message = "invalid memory map";

    debug!("physical memory:");
    for region in memory_map.iter() {
        let usable = region.region_type == MemoryRegionType::Usable;
        let start = region.range.start_frame_number;
        let end = region.range.end_frame_number;
        let physical_region = Block::<Frame>::from_index_u64(start, end).expect(message);
        let isolated = !prev_physical_region.is_adjacent(physical_region);

        debug!(
            "    {}; type = {:?}; usable = {}; isolated = {}",
            physical_region, region.region_type, usable, isolated,
        );

        if usable || !isolated {
            usable_frame_count += end - start;
            total_frame_count = cmp::max(total_frame_count, end);
        }

        if !prev_physical_region.is_disjoint(physical_region) {
            error!(%prev_physical_region, %physical_region, "memory map has intersecting regions");
        }
        if prev_physical_region > physical_region {
            warn!(%prev_physical_region, %physical_region, "unsorted memory map");
        }
        if usable && isolated {
            warn!(%physical_region, "discontinuous physical memory or unsorted memory map");
        }
        prev_physical_region = physical_region;
    }

    let physical_memory = Block::from_index_u64(0, total_frame_count).expect(message);

    info!(
        %physical_memory,
        total = %Size::new_u64::<Frame>(total_frame_count),
        usable = %Size::new_u64::<Frame>(usable_frame_count),
        total_frame_count,
        usable_frame_count,
    );

    physical_memory
}


lazy_static! {
    /// Аллокатор виртуальных страниц для ядра.
    static ref KERNEL_PAGE_ALLOCATOR: Spinlock<PageAllocator> = Spinlock::new(PageAllocator::default());
}


/// Количество записей таблицы страниц корневого уровня,
/// отвечающих одной из половин виртуального адресного пространства.
const LOWER_HALF_ROOT_LEVEL_ENTRY_COUNT: usize = PAGE_TABLE_ENTRY_COUNT / 2;

/// Количество записей таблицы страниц корневого уровня, зарезервированных для ядра.
const KERNEL_ROOT_LEVEL_ENTRY_COUNT: usize = 32;

/// Количество страниц, за которые отвечает одна запись таблицы страниц корневого уровня.
const PAGES_PER_ROOT_LEVEL_ENTRY: usize = PAGE_TABLE_ENTRY_COUNT.pow(PAGE_TABLE_ROOT_LEVEL);


const_assert!(KERNEL_ROOT_LEVEL_ENTRY_COUNT <= LOWER_HALF_ROOT_LEVEL_ENTRY_COUNT / 2);
const_assert!(
    kernel_root_level_entries().end <= user_root_level_entries().start ||
        user_root_level_entries().end <= kernel_root_level_entries().start
);
const_assert!(
    kernel_root_level_entries().end <= LOWER_HALF_ROOT_LEVEL_ENTRY_COUNT ||
        LOWER_HALF_ROOT_LEVEL_ENTRY_COUNT <= kernel_root_level_entries().start
);
const_assert!(
    user_root_level_entries().end <= LOWER_HALF_ROOT_LEVEL_ENTRY_COUNT ||
        LOWER_HALF_ROOT_LEVEL_ENTRY_COUNT <= user_root_level_entries().start
);


#[doc(hidden)]
pub mod test_scaffolding {
    use core::{
        ops::Range,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use ku::memory::mmu::PAGE_TABLE_ENTRY_COUNT;

    use crate::log::error;

    use super::{
        Block,
        BootFrameAllocator,
        Frame,
        FrameAllocator,
        Page,
        BASE_ADDRESS_SPACE,
        FRAME_ALLOCATOR,
    };

    use super::phys2virt::test_scaffolding as phys2virt;

    pub use super::{
        address_space::test_scaffolding::*,
        boot_frame_allocator::test_scaffolding::*,
        mapping::test_scaffolding::*,
        page_allocator::test_scaffolding::*,
        phys2virt::test_scaffolding::*,
    };


    #[must_use]
    pub fn forbid_frame_leaks() -> impl Drop {
        BASE_ADDRESS_SPACE.lock().mapping().unmap_unused_intermediate();

        scopeguard::guard(FRAME_ALLOCATOR.lock().count(), |start_free_frames| {
            BASE_ADDRESS_SPACE.lock().mapping().unmap_unused_intermediate();

            let end_free_frames = FRAME_ALLOCATOR.lock().count();

            let (message, affected_frames) = if start_free_frames <= end_free_frames {
                (
                    "freed more memory than allocated",
                    end_free_frames - start_free_frames,
                )
            } else {
                ("leaked some memory", start_free_frames - end_free_frames)
            };

            if affected_frames != 0 {
                error!(start_free_frames, end_free_frames, affected_frames, message);
                panic!("{}", message);
            }
        })
    }


    pub fn get_boot(frame_allocator: &mut FrameAllocator) -> Option<&mut BootFrameAllocator> {
        match frame_allocator {
            FrameAllocator::Boot(boot_allocator) => Some(boot_allocator),
            FrameAllocator::Main(..) | FrameAllocator::Void => None,
        }
    }


    pub fn frame_count() -> usize {
        FRAME_COUNT.load(Ordering::Relaxed)
    }


    pub(super) fn set_frame_count(frame_count: usize) {
        FRAME_COUNT.store(frame_count, Ordering::Relaxed);
    }


    pub const fn kernel_root_level_entries() -> Range<usize> {
        super::kernel_root_level_entries()
    }


    pub const fn user_root_level_entries() -> Range<usize> {
        super::user_root_level_entries()
    }


    pub fn user_pages() -> Block<Page> {
        super::user_pages()
    }


    pub fn physical_memory() -> Block<Frame> {
        phys2virt::physical_memory(&BASE_ADDRESS_SPACE.lock().mapping().phys2virt())
    }


    pub fn recursive_mapping() -> Option<usize> {
        let recursive_mapping = RECURSIVE_MAPPING.load(Ordering::Relaxed);
        if recursive_mapping < PAGE_TABLE_ENTRY_COUNT {
            Some(recursive_mapping)
        } else {
            None
        }
    }


    pub(super) fn set_recursive_mapping(recursive_mapping: usize) {
        RECURSIVE_MAPPING.store(recursive_mapping, Ordering::Relaxed);
    }


    pub const KERNEL_ROOT_LEVEL_ENTRY_COUNT: usize = super::KERNEL_ROOT_LEVEL_ENTRY_COUNT;
    pub const LOWER_HALF_ROOT_LEVEL_ENTRY_COUNT: usize = super::LOWER_HALF_ROOT_LEVEL_ENTRY_COUNT;
    pub const PAGES_PER_ROOT_LEVEL_ENTRY: usize = super::PAGES_PER_ROOT_LEVEL_ENTRY;


    static FRAME_COUNT: AtomicUsize = AtomicUsize::new(0);
    static RECURSIVE_MAPPING: AtomicUsize = AtomicUsize::new(usize::MAX);
}
