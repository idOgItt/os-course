use std::{
    alloc::{AllocError, Allocator, Layout},
    ffi::c_void,
    fs::File,
    num::NonZeroUsize,
    ptr::NonNull,
    result,
};

use nix::sys::mman::{self, MapFlags, ProtFlags};


pub struct MmapFallback;


unsafe impl Allocator for MmapFallback {
    fn allocate(&self, layout: Layout) -> result::Result<NonNull<[u8]>, AllocError> {
        let ptr = if layout.size() == 0 {
            layout.align() as *mut u8
        } else {
            unsafe {
                mman::mmap::<File>(
                    None,
                    NonZeroUsize::new(layout.size()).unwrap(),
                    ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                    MapFlags::MAP_ANONYMOUS | MapFlags::MAP_PRIVATE,
                    None,
                    0,
                )
                .unwrap() as *mut u8
            }
        };

        NonNull::new(ptr)
            .map(|ptr| NonNull::slice_from_raw_parts(ptr, layout.size()))
            .ok_or(AllocError)
    }


    unsafe fn deallocate(&self, mut ptr: NonNull<u8>, layout: Layout) {
        if layout.size() == 0 {
            return;
        }

        unsafe {
            mman::munmap(ptr.as_mut() as *mut u8 as *mut c_void, layout.size()).unwrap();
        }
    }
}
