use std::{alloc::Layout, ffi::c_void, fs::File, num::NonZeroUsize};

use nix::sys::mman::{self, MRemapFlags, MapFlags, ProtFlags};

use ku::{
    allocator::BigAllocator,
    error::Result,
    memory::{Block, Page, Virt},
};


pub struct MmapBig {
    reserved: Block<Page>,
    buffer: Block<Page>,
    mirrored_buffer: Block<Page>,
}


impl MmapBig {
    pub fn new() -> Self {
        Self {
            reserved: Block::default(),
            buffer: Block::default(),
            mirrored_buffer: Block::default(),
        }
    }

    pub fn unmap(self) {
        // Do not unmap the memory in the `Drop` impl.
        // A drop will happen on any panic.
        // If it unmaps the memory it can cause a SIGSEGV that hides the real problem.
        for block in [self.buffer, self.mirrored_buffer] {
            unsafe {
                mman::munmap(
                    block.start_address().try_into_mut_ptr().unwrap(),
                    block.size(),
                )
                .unwrap();
            }
        }
    }


    fn block(ptr: *mut c_void, len: usize) -> Result<Block<Page>> {
        let virt = Virt::from_ptr(ptr);
        Block::new(Page::new(virt)?, Page::new((virt + len)?)?)
    }
}


unsafe impl BigAllocator for MmapBig {
    fn reserve(&mut self, layout: Layout) -> Result<Block<Page>> {
        assert_eq!(self.reserved, Block::default());
        assert_eq!(layout.size() % (2 * Page::SIZE), 0);
        assert_eq!(layout.align() % Page::SIZE, 0);

        let ptr = unsafe {
            mman::mmap::<File>(
                None,
                NonZeroUsize::new(layout.size()).unwrap(),
                ProtFlags::PROT_NONE,
                MapFlags::MAP_ANONYMOUS | MapFlags::MAP_PRIVATE,
                None,
                0,
            )
            .unwrap()
        };

        self.reserved = Self::block(ptr, layout.size())?;

        Ok(self.reserved)
    }


    unsafe fn unreserve(&mut self, _block: Block<Page>) -> Result<()> {
        unimplemented!();
    }


    unsafe fn rereserve(&mut self, _old_block: Block<Page>, _sub_block: Block<Page>) -> Result<()> {
        unimplemented!();
    }


    unsafe fn map(&mut self, block: Block<Page>) -> Result<()> {
        assert_eq!(self.buffer, Block::default());
        assert_eq!(block.size(), self.reserved.size() / 2);
        assert!(self.reserved.contains_block(block));

        let ptr = unsafe {
            mman::mmap::<File>(
                NonZeroUsize::new(block.start_address().into_usize()),
                NonZeroUsize::new(block.size()).unwrap(),
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_ANONYMOUS | MapFlags::MAP_SHARED | MapFlags::MAP_FIXED,
                None,
                0,
            )
            .unwrap()
        };

        self.buffer = Self::block(ptr, block.size())?;

        Ok(())
    }


    unsafe fn unmap(&mut self, _block: Block<Page>) -> Result<()> {
        unimplemented!();
    }


    unsafe fn copy_mapping(
        &mut self,
        old_block: Block<Page>,
        new_block: Block<Page>,
    ) -> Result<()> {
        assert_ne!(self.buffer, Block::default());
        assert_eq!(self.mirrored_buffer, Block::default());
        assert_eq!(self.buffer, old_block);
        assert_eq!(old_block.size(), new_block.size());
        assert!(self.reserved.contains_block(new_block));

        let ptr = unsafe {
            mman::mremap(
                old_block.start_address().try_into_mut_ptr()?,
                0,
                new_block.size(),
                MRemapFlags::MREMAP_MAYMOVE | MRemapFlags::MREMAP_FIXED,
                Some(new_block.start_address().try_into_mut_ptr()?),
            )
            .unwrap()
        };

        self.mirrored_buffer = Self::block(ptr, new_block.size())?;

        Ok(())
    }
}
