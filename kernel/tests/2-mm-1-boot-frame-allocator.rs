#![deny(warnings)]
#![feature(custom_test_frameworks)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


use ku::memory::size::{MiB, Size};

use kernel::{
    error::Error::NoFrame,
    log::debug,
    memory::{
        test_scaffolding::{
            allocate_block,
            forbid_frame_leaks,
            frame_count,
            get_boot,
            is_managed,
            is_used,
        },
        Block,
        Frame,
        Phys,
        FRAME_ALLOCATOR,
    },
    Subsystems,
};


mod gen_main;


gen_main!(Subsystems::PHYS_MEMORY);


#[test_case]
fn t1_sanity_check() {
    let _guard = forbid_frame_leaks();

    let mut frame_allocator = FRAME_ALLOCATOR.lock();
    let free_frames = frame_allocator.count();
    let frame_allocator = get_boot(&mut *frame_allocator).unwrap();

    let qemu_memory_frames = 128 * MiB / Frame::SIZE;
    let min_free_frames = qemu_memory_frames - 16 * MiB / Frame::SIZE;

    debug!(free_frames, min_free_frames, qemu_memory_frames);

    assert!(free_frames > min_free_frames);
    assert!(free_frames < qemu_memory_frames);

    let mut managed = 0;
    let mut used = 0;

    for frame in Block::<Frame>::from_index(0, qemu_memory_frames).unwrap() {
        if is_managed(&*frame_allocator, frame) {
            managed += 1;
        }
        if is_used(&*frame_allocator, frame) {
            used += 1;
        }
    }

    debug!(managed, used);

    assert_eq!(managed, free_frames + used);
    assert!(used <= 1);
}


#[test_case]
fn t2_allocate_block() {
    let mut frame_allocator = FRAME_ALLOCATOR.lock();

    for size in (0..=3 * Frame::SIZE).filter(|x| (x + 3) % Frame::SIZE < 6) {
        let free_frame_count = frame_allocator.count();
        let allocated = allocate_block(get_boot(&mut *frame_allocator).unwrap(), size).unwrap();
        let allocated_frame_count = free_frame_count - frame_allocator.count();

        debug!(requested = %Size::new::<Phys>(size), %allocated, allocated_frame_count);

        assert!(
            allocated.size() >= size,
            "allocated less than the requested size",
        );
        assert!(
            allocated.size() - size < Frame::SIZE,
            "allocated excessive memory",
        );
        assert_eq!(allocated.count(), allocated_frame_count, "lost some frames");
    }
}


#[test_case]
fn t3_allocate() {
    let mut frame_allocator = FRAME_ALLOCATOR.lock();
    let start_free_frames = frame_allocator.count();
    let frame_count = frame_count();

    let frames = [
        frame_allocator.allocate().unwrap(),
        frame_allocator.allocate().unwrap(),
    ];

    debug!(?frames, frame_count);
    assert_ne!(frames[0], frames[1]);
    assert_eq!(frame_allocator.count(), start_free_frames - 2);
    for frame in frames {
        assert!(
            frame.index() < frame_count,
            "allocated a frame outside of the physical memory of the current machine",
        );
    }
}


#[test_case]
fn t4_allocated_frames_are_unique() {
    let mut frame_allocator = FRAME_ALLOCATOR.lock();
    let free_frames = frame_allocator.count();
    let frame_count = frame_count();

    debug!(free_frames, frame_count);

    let mut direction = None;
    let mut prev_frame = None;

    for i in 0..free_frames {
        let frame = frame_allocator.allocate().unwrap();
        assert!(
            frame.index() < frame_count,
            "allocated a frame {frame} outside of the physical memory of the current machine with only {frame_count} frames total",
        );

        if let Some(prev_frame) = prev_frame {
            if i % 10_000 == 1 {
                debug!(%prev_frame, %frame);
            }

            assert_ne!(prev_frame, frame, "allocated the same frame twice");

            if let Some(direction) = direction {
                assert_eq!(direction, prev_frame < frame, "expected either ascending or descending order of allocations from BootFrameAllocator");
            } else {
                direction = Some(prev_frame < frame);
            }
        }

        prev_frame = Some(frame);
    }

    assert_eq!(frame_allocator.count(), 0);
    assert_eq!(frame_allocator.allocate(), Err(NoFrame));
}
