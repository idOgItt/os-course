use core::{
    alloc::{AllocError, Allocator, Layout},
    ptr::NonNull,
    result,
};

use ku::{
    allocator::{DryAllocator, Initialize},
    memory::mmu::PageTableFlags,
    sync::spinlock::Spinlock,
};

use crate::memory::AddressSpace;


/// Аллокатор памяти общего назначения внутри [`AddressSpace`].
pub(crate) struct MemoryAllocator<'a> {
    /// Адресное пространство, внутри которого аллокатор выделяет память.
    address_space: &'a Spinlock<AddressSpace>,

    /// Флаги с которыми будет отображена выделяемая память.
    flags: PageTableFlags,
}


impl MemoryAllocator<'_> {
    /// Возвращает аллокатор памяти общего назначения внутри `address_space`.
    /// Выделяемая им память будет отображена с флагами `flags`.
    pub const fn new(
        address_space: &Spinlock<AddressSpace>,
        flags: PageTableFlags,
    ) -> MemoryAllocator {
        MemoryAllocator {
            address_space,
            flags,
        }
    }
}


unsafe impl Allocator for MemoryAllocator<'_> {
    fn allocate(&self, layout: Layout) -> result::Result<NonNull<[u8]>, AllocError> {
        self.address_space
            .lock()
            .allocator(self.flags)
            .dry_allocate(layout, Initialize::Garbage)
            .map_err(|_| AllocError)
    }


    fn allocate_zeroed(&self, layout: Layout) -> result::Result<NonNull<[u8]>, AllocError> {
        self.address_space
            .lock()
            .allocator(self.flags)
            .dry_allocate(layout, Initialize::Zero)
            .map_err(|_| AllocError)
    }


    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        unsafe {
            self.address_space.lock().allocator(self.flags).dry_deallocate(ptr, layout);
        }
    }


    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> result::Result<NonNull<[u8]>, AllocError> {
        unsafe {
            self.address_space
                .lock()
                .allocator(self.flags)
                .dry_grow(ptr, old_layout, new_layout, Initialize::Garbage)
                .map_err(|_| AllocError)
        }
    }


    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> result::Result<NonNull<[u8]>, AllocError> {
        unsafe {
            self.address_space
                .lock()
                .allocator(self.flags)
                .dry_grow(ptr, old_layout, new_layout, Initialize::Zero)
                .map_err(|_| AllocError)
        }
    }


    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> result::Result<NonNull<[u8]>, AllocError> {
        unsafe {
            self.address_space
                .lock()
                .allocator(self.flags)
                .dry_shrink(ptr, old_layout, new_layout)
                .map_err(|_| AllocError)
        }
    }
}
