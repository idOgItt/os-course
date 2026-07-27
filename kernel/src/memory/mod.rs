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

/// Реализация отображения виртуальной памяти в физическую [`Mapping`].
mod mapping;

/// Аллокатор страниц виртуальной памяти [`PageAllocator`].
mod page_allocator;

/// Работа со стеками [`Stack`] и
/// выделенные стеки для непредвиденных исключений [`ExceptionStacks`].
mod stack;

/// Сегмент состояния задачи
/// ([Task State Segment](https://en.wikipedia.org/wiki/Task_state_segment), TSS).
mod tss;


use bootloader::BootInfo;
use lazy_static::lazy_static;
use x86_64::registers::model_specific::{Efer, EferFlags};

pub use ku::{
    memory::{addr, block, frage, size},
    sync::spinlock::Spinlock,
};

use crate::{
    error::{Error::NoFrame, Result},
    log::{error, info},
    time,
    Subsystems,
};

use boot_frame_allocator::BootFrameAllocator;
use main_frame_allocator::MainFrameAllocator;
use mapping::Mapping;

pub use addr::{Phys, Virt};
pub use address_space::{AddressSpace, BASE_ADDRESS_SPACE};
pub use block::Block;
pub use frage::{Frame, Page};
pub use mmu::{KERNEL_READ, KERNEL_RW, USER, USER_READ, USER_RW};
pub use size::{Size, SizeOf};

pub(crate) use gdt::{Gdt, RealModePseudoDescriptor, GDT};
pub(crate) use stack::{Stack, EXCEPTION_STACKS};
pub(crate) use tss::{DOUBLE_FAULT_IST_INDEX, PAGE_FAULT_IST_INDEX};

// Used in docs.
#[allow(unused)]
use {crate::error::Error, page_allocator::PageAllocator, stack::ExceptionStacks};


/// Инициализация подсистемы памяти.
pub(super) fn init(boot_info: &'static BootInfo, subsystems: Subsystems) {
    let timer = time::timer();

    unsafe {
        Efer::write(EferFlags::NO_EXECUTE_ENABLE | Efer::read());
    }

    let phys2virt = phys2virt(boot_info);
    info!(?phys2virt);

    test_scaffolding::set_total_frames(&boot_info.memory_map);

    if subsystems.contains(Subsystems::PHYS_MEMORY) {
        *FRAME_ALLOCATOR.lock() =
            FrameAllocator::Boot(BootFrameAllocator::new(&boot_info.memory_map));
    }

    if subsystems.contains(Subsystems::VIRT_MEMORY) {
        let page_directory = Mapping::current_page_directory();
        *BASE_ADDRESS_SPACE.lock() = AddressSpace::new(page_directory, phys2virt);
    }

    if subsystems.contains(Subsystems::MAIN_FRAME_ALLOCATOR) {
        if let FrameAllocator::Boot(boot_frame_allocator) = &*FRAME_ALLOCATOR.lock() {
            assert!(!boot_frame_allocator.is_inconsistent());
        }

        *FRAME_ALLOCATOR.lock() =
            FrameAllocator::Main(main_frame_allocator::init(&boot_info.memory_map));
    }

    if subsystems.contains(Subsystems::VIRT_MEMORY | Subsystems::MAIN_FRAME_ALLOCATOR) {
        if let Err(error) = EXCEPTION_STACKS.lock().make_guard_zones() {
            error!(
                ?error,
                "failed to make guard zones for the exception stacks",
            );
        }
    }

    info!(duration = %timer.elapsed(), "memory init");
}


/// Возвращает виртуальную страницу `phys2virt`, с которой начинается линейное отображение
/// всей физической памяти в виртуальную.
pub(super) fn phys2virt(boot_info: &BootInfo) -> Page {
    Virt::new_u64(boot_info.physical_memory_offset)
        .and_then(Page::new)
        .expect("invalid phys2virt in BootInfo")
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


/// Для простоты работы с ней, физическая память целиком замаплена в некоторую область виртуальной.
/// Эта функция по адресу первой (виртуальной) страницы этой области `phys2virt`
/// выдаёт соответствующий виртуальный адрес для заданного физического `address`.
pub(crate) fn phys2virt_map(phys2virt: Page, address: Phys) -> Virt {
    (phys2virt.address() + address.into_usize()).expect("bad Phys address for phys2virt_map()")
}


#[doc(hidden)]
pub mod test_scaffolding {
    use core::sync::atomic::{AtomicUsize, Ordering};

    use bootloader::bootinfo::MemoryMap;

    use crate::log::error;

    use super::{
        main_frame_allocator,
        BootFrameAllocator,
        FrameAllocator,
        Page,
        Phys,
        Virt,
        FRAME_ALLOCATOR,
    };

    pub use super::{
        address_space::test_scaffolding::*,
        boot_frame_allocator::test_scaffolding::*,
        mapping::test_scaffolding::*,
    };


    #[must_use]
    pub fn forbid_frame_leaks() -> impl Drop {
        scopeguard::guard(FRAME_ALLOCATOR.lock().count(), |start_free_frames| {
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


    pub fn phys2virt_map(phys2virt: Page, address: Phys) -> Virt {
        super::phys2virt_map(phys2virt, address)
    }


    pub fn total_frames() -> usize {
        TOTAL_FRAMES.load(Ordering::Relaxed)
    }


    pub(super) fn set_total_frames(memory_map: &MemoryMap) {
        TOTAL_FRAMES.store(
            main_frame_allocator::test_scaffolding::total_frames(memory_map),
            Ordering::Relaxed,
        );
    }


    static TOTAL_FRAMES: AtomicUsize = AtomicUsize::new(0);
}
