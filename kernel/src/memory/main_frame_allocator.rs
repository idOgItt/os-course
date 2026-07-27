#![forbid(unsafe_code)]


use core::{cmp, mem, option::Option};
use core::cmp::min;
use core::ops::Range;
use bootloader::bootinfo::{MemoryMap, MemoryRegionType};

use crate::{
    error::{Error::NoFrame, Result},
    log::{debug, info, trace},
    time,
};

use super::{
    frage::Frame,
    size,
    size::Size,
    BootFrameAllocator,
    FrameAllocator,
    BASE_ADDRESS_SPACE,
    FRAME_ALLOCATOR,
    KERNEL_RW,
};

// Used in docs.
#[allow(unused)]
use {super::Mapping, crate::error::Error};


/// Информация об одном физическом фрейме.
#[derive(Clone, Debug)]
enum FrameInfo {
    /// Фрейм не доступен --- либо находится за пределами физической памяти,
    /// либо зарезервирован аппаратурой, загрузчиком или BIOS.
    Absent,

    /// Фрейм свободен.
    Free {
        /// Номер следующего свободного фрейма.
        next_free: Option<usize>,
    },

    /// Фрейм занят.
    Used {
        /// Количество ссылок на этот фрейм.
        /// Например, из одного или разных страничных отображений [`Mapping`].
        reference_count: usize,
    },
}


/// Инициализирует аллокатор [`MainFrameAllocator`] по информации из `memory_map`.
/// И передаёт под его управление все фреймы,
/// которые остались свободны в [`BootFrameAllocator`].
pub(super) fn init(memory_map: &MemoryMap) -> MainFrameAllocator {
    // ANCHOR: log_tsc
    let timer = time::timer();
    let frame_allocator = MainFrameAllocator::new(memory_map);
    info!(
        frame_allocator = "main",
        free_frame_count = frame_allocator.count(),
        duration = %timer.elapsed(),
        "init",
    );
    // ANCHOR_END: log_tsc
    frame_allocator
}


/// Основной аллокатор физических фреймов.
pub struct MainFrameAllocator {
    /// Информация про все доступные физические фреймы.
    frame_info: &'static mut [FrameInfo],

    /// Количество свободных физических фреймов.
    free_count: usize,

    /// Голова интрузивного списка номеров свободных физических фреймов.
    free_frame: Option<usize>,
}


impl MainFrameAllocator {
    /// Инициализирует аллокатор [`MainFrameAllocator`] по информации из `memory_map`.
    /// И передаёт под его управление все фреймы,
    /// которые остались свободны в [`BootFrameAllocator`].
    pub(super) fn new(memory_map: &MemoryMap) -> Self {
        let mut timer = time::timer();

        let frame_count = total_frames(memory_map);
        let frame_info = BASE_ADDRESS_SPACE
            .lock()
            .map_slice(frame_count, KERNEL_RW, || FrameInfo::Absent)
            .expect("failed to allocate memory for MainFrameAllocator metadata");

        debug!(frame_allocator = "main", duration = %timer.lap(), "frame info mapped");

        let mut main_frame_allocator = Self {
            frame_info,
            free_count: 0,
            free_frame: None,
        };

        let boot_frame_allocator = mem::replace(&mut *FRAME_ALLOCATOR.lock(), FrameAllocator::Void);

        if let FrameAllocator::Boot(boot_frame_allocator) = boot_frame_allocator {
            main_frame_allocator.init_frame_info(memory_map, &boot_frame_allocator);
            debug!(frame_allocator = "main", duration = %timer.lap(), "frame info init");

            main_frame_allocator.move_free_frames(boot_frame_allocator);
            debug!(frame_allocator = "main", duration = %timer.lap(), "moved all free frames");

            main_frame_allocator
        } else {
            panic!("FrameAllocator should be a Boot one now");
        }
    }


    /// Возвращает количество свободных физических фреймов у аллокатора [`MainFrameAllocator`].
    pub(super) fn count(&self) -> usize {
        self.free_count
    }


    /// Выделяет ровно один физический фрейм.
    /// Устанавливает количество ссылок на него равным единице.
    ///
    /// Если свободных физических фреймов не осталось,
    /// возвращает ошибку [`Error::NoFrame`].
    pub(super) fn allocate(&mut self) -> Result<Frame> {
        let free = self.free_frame.ok_or(Error::NoFrame)?;

        let frame_info = &mut self.frame_info[free];

        match frame_info {
            FrameInfo::Free { next_free } => {
                self.free_frame = *next_free;
                let frame = Frame::from_index(free).expect("Frame index should not overflow");

                *frame_info = FrameInfo::Used { reference_count: 1 };
                self.free_count -= 1;

                Ok(frame)
            },
            _ => {
                panic!("Tried to allocate a non-free frame at index {}", free)
            },
        }
    }


    /// Уменьшает на единицу счётчик использований заданного физического фрейма `frame`.
    /// Физический фрейм освобождается, если на него не осталось других ссылок.
    ///
    /// # Panics
    ///
    /// Паникует, если фрейм свободен.
    pub(super) fn deallocate(&mut self, frame: Frame) {
        let index = frame.index();

        if self.should_deallocate_after_decrementing_reference_count(index) {
            self.deallocate_frame(index);
        }
    }

    fn should_deallocate_after_decrementing_reference_count(&mut self, index: usize) -> bool {
        match &mut self.frame_info[index] {
            FrameInfo::Used { reference_count } => {
                if *reference_count == 1 {
                    true
                } else {
                    *reference_count -= 1;
                    false
                }
            },
            FrameInfo::Absent => false,
            FrameInfo::Free { .. } => {
                panic!("Tried to free an already free frame at index {}", index);
            },
        }
    }

    /// Освобождает фрейм и обновляет список свободных фреймов.
    fn deallocate_frame(&mut self, index: usize) {
        let next_free = self.get_next_free_and_insert_at(index);

        self.frame_info[index] = FrameInfo::Free { next_free };
        self.free_count += 1;
    }

    fn insert_after_free_frame_and_get_next_free(
        free_frame: &mut FrameInfo,
        index: usize,
    ) -> Option<usize> {
        match free_frame {
            FrameInfo::Free { next_free } => {
                let second_next_free = *next_free;
                *next_free = Some(index);
                second_next_free
            },
            _ => panic!("Can't insert after a non-free frame"),
        }
    }

    fn get_last_free_frame_in_range(
        &mut self,
        search_range: Range<usize>,
    ) -> Option<&mut FrameInfo> {
        self.frame_info[search_range]
            .iter_mut()
            .rev()
            .filter(|f| matches!(f, FrameInfo::Free { .. }))
            .next()
    }

    fn get_next_free_and_insert_at(&mut self, index: usize) -> Option<usize> {
        match self.free_frame {
            Some(free_frame) => {
                let previous_free_frame_range = min(free_frame, index)..index;

                if let Some(previous) = self.get_last_free_frame_in_range(previous_free_frame_range)
                {
                    Self::insert_after_free_frame_and_get_next_free(previous, index)
                } else {
                    let next_free = self.free_frame;
                    self.free_frame = Some(index);
                    next_free
                }
            },
            None => {
                self.free_frame = Some(index);
                None
            },
        }
    }


    /// Увеличивает на единицу счётчик использований заданного физического фрейма `frame`.
    ///
    /// # Panics
    ///
    /// Паникует, если фрейм свободен.
    pub(super) fn reference(&mut self, frame: Frame) {
        match &mut self.frame_info[frame.index()] {
            FrameInfo::Used { reference_count } => *reference_count += 1,
            FrameInfo::Free { .. } => panic!("Tried to reference a free frame"),
            _ => {},
        }
    }


    /// Возвращает количество ссылок на `frame`.
    ///
    /// - Если `frame` свободен, возвращается `0`.
    /// - Если он отсутствует или зарезервирован --- [`Error::NoFrame`].
    pub(super) fn reference_count(&self, frame: Frame) -> Result<usize> {
        match self.frame_info[frame.index()] {
            FrameInfo::Absent => Err(NoFrame),
            FrameInfo::Free { .. } => Ok(0),
            FrameInfo::Used { reference_count } => Ok(reference_count),
        }
    }


    /// Проверяет, что заданный физический фрейм уже был аллоцирован.
    pub(super) fn is_used(&self, frame: Frame) -> bool {
        if frame.index() >= self.frame_info.len() {
            true
        } else {
            !matches!(self.frame_info[frame.index()], FrameInfo::Free { .. })
        }
    }


    /// Инициализирует аллокатор [`MainFrameAllocator`] по информации из `memory_map`.
    /// Отмечает все фреймы, которые находятся под управлением [`BootFrameAllocator`]
    /// как занятые со счётчиком использований, равным 1.
    fn init_frame_info(
        &mut self,
        memory_map: &MemoryMap,
        boot_frame_allocator: &BootFrameAllocator,
    ) {
        let mut usable_frames_rev = memory_map
            .iter()
            .map(|region| {
                let start = size::from(region.range.start_frame_number);
                let end = size::from(region.range.end_frame_number);

                (region.region_type, start..end)
            })
            .filter(|(region_type, _)| *region_type == MemoryRegionType::Usable)
            .rev()
            .flat_map(|(_, range)| range.rev().map(|i| Frame::from_index(i).unwrap()));

        let mut next_free: Option<usize> = None;

        while let Some(frame) = usable_frames_rev.next() {
            let used = BootFrameAllocator::is_managed(boot_frame_allocator, frame);
            let frame_info = &mut self.frame_info[frame.index()];

            if used {
                *frame_info = FrameInfo::Used { reference_count: 1 };
            } else {
                *frame_info = FrameInfo::Free { next_free };
                next_free = Some(frame.index());
                self.free_count += 1;
            }
        }

        self.free_frame = next_free;
    }


    /// И передаёт под управление [`MainFrameAllocator`] все фреймы,
    /// которые на данный момент принадлежат [`BootFrameAllocator`] и
    /// являются свободными.
    fn move_free_frames(&mut self, mut src: BootFrameAllocator) {
        let unallocated_size = src.count() * Frame::SIZE;
        if unallocated_size != 0 {
            let block = BootFrameAllocator::allocate_block(&mut src, unallocated_size).unwrap();

            for frame in block {
                self.deallocate(frame);
            }
        }
    }
}


/// Возвращает размер среза, достаточного для отслеживания всех незарезервированных фреймов.
fn total_frames(memory_map: &MemoryMap) -> usize {
    let mut total_frames = 0;
    let mut usable_frames = 0;

    for region in memory_map.iter() {
        let usable = region.region_type == MemoryRegionType::Usable;
        let start = region.range.start_frame_number;
        let end = region.range.end_frame_number;

        if usable {
            usable_frames += end - start;
            total_frames = cmp::max(total_frames, end);
        }
    }

    info!(
        total = %Size::new_u64::<Frame>(total_frames),
        usable = %Size::new_u64::<Frame>(usable_frames),
        total_frames,
        usable_frames,
        "available memory",
    );

    size::into_usize(total_frames)
}


impl Drop for MainFrameAllocator {
    fn drop(&mut self) {
        panic!("can not drop MainFrameAllocator");
    }
}


#[doc(hidden)]
pub(super) mod test_scaffolding {
    use bootloader::bootinfo::MemoryMap;


    pub(in super::super) fn total_frames(memory_map: &MemoryMap) -> usize {
        super::total_frames(memory_map)
    }
}
