#![forbid(unsafe_code)]


use bootloader::bootinfo::{MemoryMap, MemoryRegionType};

use crate::{
    error::{Error::NoFrame, Result},
    log::{info, trace},
};

use super::{block::Block, frage::Frame, size::SizeOf};

// Used in docs.
#[allow(unused)]
use crate::error::Error;


/// Вспомогательный аллокатор физических фреймов,
/// использующийся до инициализации основного [`super::main_frame_allocator::MainFrameAllocator`].
/// Выделяет фреймы из блока подряд и не способен забирать их назад (освобождать).
pub struct BootFrameAllocator {
    /// Блок свободных физических фреймов.
    block: Block<Frame>,

    /// Количество вызовов деаллокации фреймов.
    deallocations: usize,

    /// Используется в методах [`BootFrameAllocator::is_managed()`] и
    /// [`BootFrameAllocator::is_used()`] для определения,
    /// что заданный физический фрейм принадлежит этому аллокатору.
    /// Вне зависимости от того был ли он уже аллоцирован или нет.
    initial_block: Block<Frame>,

    /// Количество увеличений ссылок на уже выделенные фреймы.
    references: usize,
}


impl BootFrameAllocator {
    /// Инициализирует аллокатор [`BootFrameAllocator`]
    /// самым большим блоком подряд идущих свободных физических фреймов
    /// по информации из `memory_map`.
    pub(super) fn new(memory_map: &MemoryMap) -> Self {
        let mut largest_block: Option<Block<Frame>> = None;
        let mut max_frame_count = 0;

        for region in memory_map.iter() {
            if region.region_type == MemoryRegionType::Usable {
                let start = region.range.start_frame_number as usize;
                let end = region.range.end_frame_number as usize;
                let number_frames = end - start;

                if number_frames > max_frame_count {
                    max_frame_count = number_frames;
                    largest_block = Some(Block::from_index(start, end).expect("Failed to create block"));
                }
            }
        }

        if let Some(block) = largest_block {
            Self {
                block,
                initial_block : block,
                deallocations: 0,
                references: 0,
            }
        } else {
            panic!("No usable memory regions found!");
        }
    }


    /// Возвращает количество "свободных" физических фреймов, считая, в том числе:
    ///   - утёкшие при деаллокации [`BootFrameAllocator::deallocate()`],
    ///   - фреймы для которых было "увеличено количество ссылок" в [`BootFrameAllocator::reference()`].
    pub(super) fn count(&self) -> usize {
        self.block.count() + self.deallocations - self.references
    }


    /// Выделяет блок подряд идущих фреймов, достаточный для хранения объекта
    /// размером `size` **байт**.
    ///
    /// Если выделить заданный размер физической памяти не удалось,
    /// возвращает ошибку [`Error::NoFrame`].
    pub(super) fn allocate_block(&mut self, size: usize) -> Result<Block<Frame>> {
        let frame_count = Frame::count_up(size);
        self.block.tail(frame_count).ok_or(NoFrame)
    }


    /// Выделяет ровно один физический фрейм.
    ///
    /// Если свободных физических фреймов не осталось,
    /// возвращает ошибку [`Error::NoFrame`].
    pub(super) fn allocate(&mut self) -> Result<Frame> {
        let block = self.allocate_block(1)?;
        Ok(Frame::from_index(block.start()).expect("Failed to allocate"))
    }


    /// Увеличивает на единицу общий счётчик деаллоцированных фреймов.
    /// Счетчик учитывается в [`BootFrameAllocator::count()`].
    /// Физический фрейм `_frame` утекает.
    /// Поэтому этот метод должен использоваться только в тестах.
    pub(super) fn deallocate(&mut self, _frame: Frame) {
        self.deallocations += 1;
    }


    /// Увеличивает на единицу общий счётчик ссылок на фреймы.
    /// Счетчик учитывается в [`BootFrameAllocator::count()`].
    /// Не увеличивает счётчик использования самого фрейма `_frame`.
    /// Поэтому этот метод должен использоваться только в тестах.
    pub(super) fn reference(&mut self, _frame: Frame) {
        self.references += 1;
    }


    /// Возвращает `true`, если были вызовы
    /// [`BootFrameAllocator::deallocate()`] или [`BootFrameAllocator::reference()`],
    /// после которых информация об использовании физических фреймов может быть неконсистентна.
    pub(super) fn is_inconsistent(&self) -> bool {
        self.deallocations > 0 || self.references > 0
    }


    /// Проверяет, что заданный физический фрейм управляется этим аллокатором.
    pub(super) fn is_managed(&self, frame: Frame) -> bool {
        self.initial_block.contains(frame)
    }


    /// Проверяет, что заданный физический фрейм уже был аллоцирован, причём этим аллокатором.
    pub(super) fn is_used(&self, frame: Frame) -> bool {
        self.is_managed(frame) && !self.block.contains(frame)
    }
}


impl Drop for BootFrameAllocator {
    fn drop(&mut self) {
        let leaked_frame_count = self.count();
        info!(frame_allocator = "boot", block = %self.block, leaked_frame_count, "drop");
        assert!(leaked_frame_count == 0, "leaked some memory frames");
    }
}


#[doc(hidden)]
pub(super) mod test_scaffolding {
    use crate::error::Result;

    use super::{
        super::{Block, Frame},
        BootFrameAllocator,
    };


    pub fn allocate_block(allocator: &mut BootFrameAllocator, size: usize) -> Result<Block<Frame>> {
        allocator.allocate_block(size)
    }


    pub fn is_managed(allocator: &BootFrameAllocator, frame: Frame) -> bool {
        allocator.is_managed(frame)
    }


    pub fn is_used(allocator: &BootFrameAllocator, frame: Frame) -> bool {
        allocator.is_used(frame)
    }
}
