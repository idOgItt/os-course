#[macro_export]
macro_rules! gen_main {
    ($subsystems:expr) => {
        use core::panic::PanicInfo;

        use bootloader::{entry_point, BootInfo};

        entry_point!(test_entry);


        fn test_entry(boot_info: &'static BootInfo) -> ! {
            kernel::init_subsystems(&boot_info, $subsystems);
            test_main();
            panic!("should not return to test_entry()")
        }


        #[panic_handler]
        fn panic(info: &PanicInfo) -> ! {
            ku::sync::start_panicing();
            kernel::fail_test(info)
        }
    };
}
