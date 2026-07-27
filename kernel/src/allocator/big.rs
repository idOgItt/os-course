use core::alloc::Layout;

use ku::{
    allocator::BigAllocator,
    error::{
        Error::{InvalidArgument, WrongAlignment},
        Result,
    },
    memory::{mmu::PageTableFlags, Block, Page},
};

use crate::{
    log::warn,
    memory::{AddressSpace, FRAME_ALLOCATOR},
};


// Used in docs.
#[allow(unused)]
use crate::error::Error;


/// Аллокатор памяти, предназначенный для больших аллокаций.
/// Выделяет память блоками страниц.
pub struct Big<'a> {
    /// Адресное пространство, внутри которого аллокатор выделяет память.
    address_space: &'a mut AddressSpace,

    /// Флаги доступа к выделяемой аллокатором памяти.
    flags: PageTableFlags,
}


impl Big<'_> {
    /// Возвращает аллокатор памяти для постраничных аллокаций внутри `address_space`.
    /// Выделяемая им память будет отображена с флагами `flags`.
    pub fn new(address_space: &mut AddressSpace, flags: PageTableFlags) -> Big {
        Big {
            address_space,
            flags,
        }
    }
}


unsafe impl BigAllocator for Big<'_> {
    fn reserve(&mut self, layout: Layout) -> Result<Block<Page>> {
        if layout.align() > Page::SIZE {
            warn!(?layout, page_size = %Page::SIZE, "can not handle alignments greater than the page size");
            return Err(WrongAlignment);
        }

        self.address_space.allocate(layout.size(), self.flags)
    }


    unsafe fn unreserve(&mut self, _block: Block<Page>) -> Result<()> {
        Ok(())
    }


    unsafe fn rereserve(&mut self, old_block: Block<Page>, sub_block: Block<Page>) -> Result<()> {
        if old_block.contains_block(sub_block) {
            Ok(())
        } else {
            Err(InvalidArgument)
        }
    }


    unsafe fn map(&mut self, block: Block<Page>) -> Result<()> {
        unsafe { self.address_space.map_block(block, self.flags) }
    }


    unsafe fn unmap(&mut self, block: Block<Page>) -> Result<()> {
        unsafe { self.address_space.unmap_block(block) }
    }


    unsafe fn copy_mapping(
        &mut self,
        old_block: Block<Page>,
        new_block: Block<Page>,
    ) -> Result<()> {
        // TODO: your code here.
        unimplemented!();
    }
}
