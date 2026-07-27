use x86::tlb;

pub use ku::memory::mmu::{
    page_table_root,
    set_page_table_root,
    PageTable,
    PageTableEntry,
    PageTableFlags,
    FULL_ACCESS,
    KERNEL_READ,
    KERNEL_RW,
    PAGE_OFFSET_BITS,
    PAGE_TABLE_ENTRY_COUNT,
    PAGE_TABLE_INDEX_BITS,
    PAGE_TABLE_INDEX_MASK,
    PAGE_TABLE_LEAF_LEVEL,
    PAGE_TABLE_LEVEL_COUNT,
    PAGE_TABLE_ROOT_LEVEL,
    USER,
    USER_READ,
    USER_RW,
};


use super::frage::Page;


/// Сбрасывает кеш страничного преобразования процессора
/// ([Translation Lookaside Buffer, TLB](https://en.wikipedia.org/wiki/Translation_lookaside_buffer))
/// для заданной страницы `page`.
///
/// # Safety
///
/// Вызывающий код должен работать в режиме ядра
/// (в [кольце защиты](https://en.wikipedia.org/wiki/Protection_ring) 0).
pub unsafe fn flush(page: Page) {
    unsafe {
        tlb::flush(page.address().into_usize());
    }
}
