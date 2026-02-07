use acpi::{AcpiTables, Handler};
use x86_64::VirtAddr;

#[derive(Clone)]
pub struct AcpiHandler {
    physical_memory_offset: VirtAddr,
}

impl AcpiHandler {
    pub fn new(physical_memory_offset: VirtAddr) -> Self {
        AcpiHandler {
            physical_memory_offset,
        }
    }
}

impl Handler for AcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        let virtual_address = self.physical_memory_offset.as_u64() + physical_address as u64;
        let ptr = virtual_address as *mut T;

        acpi::PhysicalMapping {
            physical_start: physical_address,
            virtual_start: core::ptr::NonNull::new(ptr).unwrap(),
            region_length: size,
            mapped_length: size,
            handler: self.clone(),
        }
    }

    fn unmap_physical_region<T>(_region: &acpi::PhysicalMapping<Self, T>) {
        // Nothing to do here for our simple identity mapping
    }

    fn nanos_since_boot(&self) -> u64 {
        0 // TODO: Implement timer to get actual uptime
    }

    fn breakpoint(&self) {
        panic!("ACPI breakpoint called"); // TODO: Implement proper breakpoint handling
    }

    fn handle_debug(&self, _object: &acpi::aml::object::Object) {
        // TODO: Implement proper debug handling
    }

    fn handle_fatal_error(&self, fatal_type: u8, fatal_code: u32, fatal_arg: u64) {
        panic!(
            "ACPI Fatal Error: type {}, code {}, arg {}",
            fatal_type, fatal_code, fatal_arg
        );
    }

    fn create_mutex(&self) -> acpi::Handle {
        // TODO: Implement proper mutex handling
        unimplemented!()
    }

    fn acquire(&self, _mutex: acpi::Handle, _timeout: u16) -> Result<(), acpi::aml::AmlError> {
        // TODO: Implement proper mutex handling
        unimplemented!()
    }

    fn release(&self, _mutex: acpi::Handle) {
        // TODO: Implement proper mutex handling
        unimplemented!()
    }

    fn stall(&self, _microseconds: u64) {
        // TODO: Implement proper stall
        unimplemented!()
    }

    fn sleep(&self, _milliseconds: u64) {
        // TODO: Implement proper sleep
        unimplemented!()
    }

    fn read_io_u16(&self, port: u16) -> u16 {
        unsafe { x86_64::instructions::port::Port::new(port).read() }
    }

    fn read_io_u32(&self, port: u16) -> u32 {
        unsafe { x86_64::instructions::port::Port::new(port).read() }
    }

    fn read_io_u8(&self, port: u16) -> u8 {
        unsafe { x86_64::instructions::port::Port::new(port).read() }
    }

    fn read_pci_u16(&self, _adress: acpi::PciAddress, _offset: u16) -> u16 {
        unimplemented!()
    }

    fn read_pci_u32(&self, _adress: acpi::PciAddress, _offset: u16) -> u32 {
        unimplemented!()
    }

    fn read_pci_u8(&self, _adress: acpi::PciAddress, _offset: u16) -> u8 {
        unimplemented!()
    }

    fn read_u16(&self, adress: usize) -> u16 {
        unsafe { core::ptr::read_volatile(adress as *const u16) }
    }

    fn read_u32(&self, adress: usize) -> u32 {
        unsafe { core::ptr::read_volatile(adress as *const u32) }
    }

    fn read_u64(&self, adress: usize) -> u64 {
        unsafe { core::ptr::read_volatile(adress as *const u64) }
    }

    fn read_u8(&self, adress: usize) -> u8 {
        unsafe { core::ptr::read_volatile(adress as *const u8) }
    }

    fn write_io_u16(&self, port: u16, value: u16) {
        unsafe { x86_64::instructions::port::Port::new(port).write(value) }
    }

    fn write_io_u32(&self, port: u16, value: u32) {
        unsafe { x86_64::instructions::port::Port::new(port).write(value) }
    }

    fn write_io_u8(&self, port: u16, value: u8) {
        unsafe { x86_64::instructions::port::Port::new(port).write(value) }
    }

    fn write_pci_u16(&self, _adress: acpi::PciAddress, _offset: u16, _value: u16) {
        // TODO: Implement PCI write
        unimplemented!()
    }

    fn write_pci_u32(&self, _address: acpi::PciAddress, _offset: u16, _value: u32) {
        unimplemented!()
    }

    fn write_pci_u8(&self, _address: acpi::PciAddress, _offset: u16, _value: u8) {
        unimplemented!()
    }

    fn write_u16(&self, address: usize, value: u16) {
        unsafe { core::ptr::write_volatile(address as *mut u16, value) }
    }

    fn write_u32(&self, address: usize, value: u32) {
        unsafe { core::ptr::write_volatile(address as *mut u32, value) }
    }

    fn write_u64(&self, address: usize, value: u64) {
        unsafe { core::ptr::write_volatile(address as *mut u64, value) }
    }

    fn write_u8(&self, address: usize, value: u8) {
        unsafe { core::ptr::write_volatile(address as *mut u8, value) }
    }
}

pub fn read_acpi_tables(
    rsdp_addr: usize,
    physical_memory_offset: VirtAddr,
) -> AcpiTables<AcpiHandler> {
    let handler = AcpiHandler::new(physical_memory_offset);

    unsafe { AcpiTables::from_rsdp(handler, rsdp_addr).unwrap() }
}
