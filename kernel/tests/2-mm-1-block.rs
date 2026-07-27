#![deny(warnings)]
#![feature(custom_test_frameworks)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


use kernel::{
    log::debug,
    memory::{Block, Virt},
    Subsystems,
};


mod gen_main;

gen_main!(Subsystems::empty());


#[test_case]
fn block() {
    for start in 0..100 {
        for count in 0..100 {
            let end = start + count;
            let mut block = Block::<Virt>::from_index(start, end).unwrap();

            if start % 25 == 0 && end % 33 == 0 {
                debug!(start, end, %block);
            }

            assert!(block.count() == count);

            for requested_count in count + 1..count + 3 {
                assert!(block.tail(requested_count).is_none());
            }

            let mut requested_count = 0;

            while requested_count < block.count() {
                let old_count = block.count();
                let old_end = block.end_address().unwrap().into_usize();

                block.tail(0).unwrap();
                assert!(block.count() == old_count);

                let b = block.tail(requested_count).unwrap();

                assert!(block.count() == old_count - requested_count);
                assert!(b.count() == requested_count);

                assert!(block.start_address().into_usize() == start);
                assert!(
                    block.start_address().into_usize() < block.end_address().unwrap().into_usize(),
                );
                assert!(
                    block.start_address().into_usize() + old_count - requested_count ==
                        block.end_address().unwrap().into_usize(),
                );
                assert!(
                    block.end_address().unwrap().into_usize() == b.start_address().into_usize(),
                );
                assert!(b.start_address().into_usize() <= b.end_address().unwrap().into_usize());
                assert!(
                    b.start_address().into_usize() + requested_count ==
                        b.end_address().unwrap().into_usize(),
                );
                assert!(b.end_address().unwrap().into_usize() <= end);
                assert!(b.end_address().unwrap().into_usize() == old_end);

                requested_count += 1;
            }

            if count > 0 {
                let remaining_count = block.count();
                assert!(remaining_count > 0);

                let c = block.tail(remaining_count).unwrap();

                assert!(block.count() == 0);
                assert!(c.count() == remaining_count);

                assert!(block.start_address().into_usize() == start);
                assert!(
                    block.start_address().into_usize() == block.end_address().unwrap().into_usize(),
                );
                assert!(
                    block.end_address().unwrap().into_usize() == c.start_address().into_usize(),
                );
                assert!(c.start_address().into_usize() < c.end_address().unwrap().into_usize());
                assert!(
                    c.start_address().into_usize() + remaining_count ==
                        c.end_address().unwrap().into_usize(),
                );
                assert!(c.end_address().unwrap().into_usize() <= end);
            }

            block.tail(0).unwrap();
            assert!(block.count() == 0);
            assert!(block.tail(1).is_none());
        }
    }
}
