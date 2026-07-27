#![allow(dead_code)]


use core::sync::atomic::{AtomicUsize, Ordering};

use ku::{
    log::debug,
    memory::{
        mmu::{PageTableFlags, PAGE_TABLE_ENTRY_COUNT, PAGE_TABLE_ROOT_LEVEL},
        Page,
        Virt,
        KERNEL_READ,
        USER_READ,
    },
};

use kernel::memory::test_scaffolding;


pub(super) fn unique_kernel_virt() -> Virt {
    unique_virt(KERNEL_READ)
}


pub(super) fn unique_user_virt() -> Virt {
    unique_virt(USER_READ)
}


pub(super) fn unique_virt(flags: PageTableFlags) -> Virt {
    const ROOT_LEVEL_ENTRY_SIZE: usize =
        PAGE_TABLE_ENTRY_COUNT.pow(PAGE_TABLE_ROOT_LEVEL) * Page::SIZE;

    static UNIQUE_ROOT_LEVEL_ENTRY: AtomicUsize = AtomicUsize::new(1);

    let is_user = flags.contains(PageTableFlags::USER_ACCESSIBLE);
    let start = test_scaffolding::user_pages().start_address();
    let offset = UNIQUE_ROOT_LEVEL_ENTRY.fetch_add(1, Ordering::Relaxed) * ROOT_LEVEL_ENTRY_SIZE;
    let virt = (if is_user {
        start + offset
    } else {
        start - offset
    })
    .unwrap();

    debug!(%virt, is_user, "allocated unique virtual address");

    virt
}
