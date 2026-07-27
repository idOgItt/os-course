use itertools::Itertools;

use crate::{
    error::{Error::NoPage, Result},
    log::{error, info, trace},
};

use super::{
    block::Block,
    frage::Page,
    mmu::{PageTable, PAGE_TABLE_ENTRY_COUNT, PAGE_TABLE_ROOT_LEVEL},
    size::SizeOf,
    stack::Stack,
};

// Used in docs.
#[allow(unused)]
use crate::error::Error;


/// Простой аллокатор виртуальных страниц адресного пространства.
#[derive(Debug, Default)]
pub(super) struct PageAllocator {
    /// Блок свободных виртуальных страниц.
    block: Block<Page>,
}


impl PageAllocator {
    /// Инициализирует аллокатор [`PageAllocator`]
    /// большим блоком подряд идущих свободных виртуальных страниц
    /// по информации из корневого узла таблицы страниц `page_directory`.
    pub(super) fn new(page_directory: &PageTable) -> Self {
        if let Some(block) = Self::find_unused_block(page_directory) {
            let mut page_allocator = Self { block };

            info!(free_page_count = page_allocator.block.count(), block = %page_allocator.block, "page allocator init");

            // Detect stack overruns that corrupt memory allocated by PageAllocator.
            page_allocator
                .allocate(Stack::GUARD_ZONE_SIZE)
                .expect("failed to create guard zone for PageAllocator");

            page_allocator
        } else {
            error!(?page_directory);
            panic!(
                "failed to find a free entry in the page directory for the virtual page allocator"
            );
        }
    }


    /// Возвращает пустой [`PageAllocator`].
    /// В отличие от [`PageAllocator::default()`] доступна в константном контексте.
    pub(super) const fn zero() -> Self {
        Self {
            block: Block::zero(),
        }
    }


    /// Выделяет блок подряд идущих виртуальных страниц, достаточный для хранения объекта
    /// размером `size` **байт**.
    /// Ни выделения физической памяти, ни создания отображения станиц, не происходит.
    ///
    /// Если выделить заданный размер виртуальной памяти не удалось,
    /// возвращает ошибку [`Error::NoPage`].
    pub(super) fn allocate(&mut self, size: usize) -> Result<Block<Page>> {
        // TODO: your code here.
        unimplemented!();
    }


    /// Возвращает блок виртуальных страниц,
    /// соответствующий самой длинной последовательности пустых записей в
    /// корневом узле таблицы страниц `page_directory`.
    ///
    /// Для упрощения не смотрит в другие узлы таблицы страниц,
    /// и резервирует только страницы, соответствующие полностью свободным
    /// записям таблицы страниц корневого уровня.
    /// Кроме того, возвращает блок страниц, целиком лежащий в одной из
    /// [канонических половин](https://en.wikipedia.org/wiki/X86-64#Virtual_address_space_details)
    /// виртуального адресного пространства.
    ///
    /// Если все записи заняты, возвращает `None`.
    fn find_unused_block(page_directory: &PageTable) -> Option<Block<Page>> {
        // TODO: your code here.
        unimplemented!();
    }
}


#[doc(hidden)]
pub(super) mod test_scaffolding {
    use super::{
        super::{Block, Page},
        PageAllocator,
    };


    pub(in super::super) fn block(page_allocator: &PageAllocator) -> Block<Page> {
        page_allocator.block
    }
}
