#![deny(warnings)]
#![feature(custom_test_frameworks)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(kernel::test_runner)]


use kernel::{
    log::info,
    memory::{mmu::PageTableFlags, test_scaffolding::mapping, Phys, BASE_ADDRESS_SPACE},
    smp::test_scaffolding::{id, local_apic},
    Subsystems,
};


mod gen_main;

gen_main!(Subsystems::MEMORY | Subsystems::LOCAL_APIC);


#[test_case]
fn mapped_properly() {
    let local_apic = local_apic();
    let expected_local_apic_address = Phys::new(0xFEE00000).unwrap();

    let mut address_space = BASE_ADDRESS_SPACE.lock();
    let mapping = mapping(&mut address_space);
    let mapping_error = "Local APIC is not mapped";
    let pte = mapping.translate(local_apic).expect(mapping_error);
    let frame = pte.frame().expect(mapping_error);
    let flags = pte.flags();

    info!(%local_apic, ?frame, ?flags, "Local APIC");

    assert!(pte.present(), "{}", mapping_error);
    assert_eq!(
        frame.address(),
        expected_local_apic_address,
        "wrong physical address for the Local APIC",
    );
    assert!(
        flags.contains(PageTableFlags::NO_CACHE | PageTableFlags::WRITE_THROUGH),
        "wrong flags for the Local APIC virtual page",
    );

    info!(cpu = id());
}
