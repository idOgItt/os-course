use core::ops::Range;

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
    PAGES_PER_ROOT_LEVEL_ENTRY,
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
    /// по информации из корневого узла таблицы страниц `page_table_root`.
    /// При этом `allowed_elements` задаёт диапазон записей корневого узла таблицы страниц,
    /// который можно использовать данному аллокатору.
    pub(super) fn new(page_table_root: &PageTable, allowed_elements: Range<usize>) -> Self {
        if let Some(block) = Self::find_unused_block(page_table_root, allowed_elements) {
            let mut page_allocator = Self { block };

            info!(free_page_count = page_allocator.block.count(), block = %page_allocator.block, "page allocator init");

            // Detect stack overruns that corrupt memory allocated by PageAllocator.
            page_allocator
                .allocate(Stack::GUARD_ZONE_SIZE)
                .expect("failed to create guard zone for PageAllocator");

            page_allocator
        } else {
            error!(?page_table_root);
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
        let page_count = Page::count_up(size);
        self.block.tail(page_count).ok_or(NoPage)
    }


    /// Возвращает блок виртуальных страниц,
    /// соответствующий самой длинной последовательности пустых записей в
    /// диапазоне `allowed_elements` корневого узла таблицы страниц `page_table_root`.
    ///
    /// Для упрощения не смотрит в другие узлы таблицы страниц,
    /// и резервирует только страницы, соответствующие полностью свободным
    /// записям таблицы страниц корневого уровня.
    ///
    /// Если все записи заняты, возвращает `None`.
    fn find_unused_block(
        page_table_root: &PageTable,
        allowed_elements: Range<usize>,
    ) -> Option<Block<Page>> {
        let mut longest_free_start = None;
        let mut longest_free_length = 0;

        let mut current_free_start = None;
        let mut cuurent_free_length = 0;

        for i in allowed_elements.clone() {
            let entry = &page_table_root[i];

            if !entry.present() {
                if current_free_start.is_none() {
                    current_free_start = Some(i);
                }
                cuurent_free_length += 1;
            } else {
                if cuurent_free_length > longest_free_length {
                    longest_free_start = current_free_start;
                    longest_free_length = cuurent_free_length;
                }
                current_free_start = None;
                cuurent_free_length = 0;
            }
        }

        if cuurent_free_length > longest_free_length {
            longest_free_start = current_free_start;
            longest_free_length = cuurent_free_length;
        }

        if let Some(start_index) = longest_free_start {
            let start_page = start_index * PAGES_PER_ROOT_LEVEL_ENTRY;
            let end_page = start_page + longest_free_length * PAGES_PER_ROOT_LEVEL_ENTRY;
            return Some(Block::from_index(start_page, end_page).expect("Fail to allocate virtual page"));
        }
        None
    }
}


#[doc(hidden)]
pub(super) mod test_scaffolding {
    use core::ops::Range;

    use super::{
        super::{mmu::PageTable, Block, Page},
        PageAllocator,
    };


    pub(in super::super) fn block(page_allocator: &PageAllocator) -> Block<Page> {
        page_allocator.block
    }


    pub fn find_unused_block(
        page_table_root: &PageTable,
        allowed_elements: Range<usize>,
    ) -> Option<Block<Page>> {
        PageAllocator::find_unused_block(page_table_root, allowed_elements)
    }
}
