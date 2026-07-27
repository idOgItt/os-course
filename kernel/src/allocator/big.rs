use core::alloc::Layout;

use ku::{
    allocator::{BigAllocator, DryAllocator},
    error::{
        Error::{InvalidArgument, WrongAlignment},
        Result,
    },
    memory::{mmu::PageTableFlags, Block, Frame, Page},
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
    fn flags(&self) -> PageTableFlags {
        self.flags
    }


    fn set_flags(&mut self, flags: PageTableFlags) -> Result<()> {
        self.flags = flags;
        Ok(())
    }


    fn reserve(&mut self, layout: Layout) -> Result<Block<Page>> {
        if layout.align() > Page::SIZE {
            warn!(?layout, page_size = %Page::SIZE, "can not handle alignments greater than the page size");
            return Err(WrongAlignment);
        }

        self.address_space.allocate(layout.size(), self.flags)
    }


    fn reserve_fixed(&mut self, block: Block<Page>) -> Result<()> {
        self.address_space.reserve(block, self.flags)
    }


    unsafe fn unreserve(&mut self, block: Block<Page>) -> Result<()> {
        self.address_space.deallocate(block)
    }


    unsafe fn rereserve(&mut self, old_block: Block<Page>, sub_block: Block<Page>) -> Result<()> {
        if old_block.contains_block(sub_block) {
            let message = "subblock of a valid block should be valid";
            let left = Block::from_index(old_block.start(), sub_block.start()).expect(message);
            let right = Block::from_index(sub_block.end(), old_block.end()).expect(message);

            if !left.is_empty() {
                unsafe {
                    self.unreserve(left)?;
                }
            }

            if !right.is_empty() {
                unsafe {
                    self.unreserve(right)?;
                }
            }

            Ok(())
        } else {
            Err(InvalidArgument)
        }
    }


    unsafe fn map(&mut self, block: Block<Page>, flags: PageTableFlags) -> Result<()> {
        unsafe { self.address_space.map_block(block, flags) }
    }


    unsafe fn unmap(&mut self, block: Block<Page>) -> Result<()> {
        unsafe { self.address_space.unmap_block(block) }
    }


    unsafe fn copy_mapping(
        &mut self,
        old_block: Block<Page>,
        new_block: Block<Page>,
        flags: Option<PageTableFlags>,
    ) -> Result<()> {
        // Check that the block sizes match
        if old_block.size() != new_block.size() {
            return Err(ku::Error::InvalidArgument);
        }

        // Determine if the blocks overlap
        let blocks_overlap = old_block.end() > new_block.start() && new_block.end() > old_block.start();

        // If blocks are identical and flags are specified, update flags for each page
        if let Some(new_flags) = flags {
            if old_block == new_block {
                for page in old_block {
                    let pte = self.address_space.mapping().translate(page.address())?;
                    pte.set_flags(new_flags);
                }
                return Ok(());
            }
        }

        // If blocks overlap, return an error
        if blocks_overlap {
            return Err(ku::Error::InvalidArgument);
        }

        // Iterate over pages in both blocks simultaneously, mapping each page in the new block
        for (old_page, new_page) in old_block.into_iter().zip(new_block.into_iter()) {
            let pte = self.address_space.mapping().translate(old_page.address())?;
            assert!(pte.present(), "Page table entry must be present.");

            let frame: Frame = pte.frame()?;
            let effective_flags = flags.unwrap_or_else(|| pte.flags());

            // Perform the mapping and reference increment
            unsafe {
                self.address_space.map_page_to_frame(new_page, frame, effective_flags)?;
                FRAME_ALLOCATOR.lock().reference(frame);
            }
        }

        Ok(())
    }
}