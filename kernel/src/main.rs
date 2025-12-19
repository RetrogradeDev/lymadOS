#![no_std]
#![no_main]

extern crate alloc;

#[cfg(not(test))]
use core::panic::PanicInfo;

use alloc::boxed::Box;
use bootloader_api::{BootInfo, BootloaderConfig, config::Mapping, entry_point};

use kernel::{
    mm::{
        allocator,
        memory::{BootInfoFrameAllocator, translate_addr},
    },
    serial_println,
};
use x86_64::{
    VirtAddr,
    structures::paging::{PageTable, Translate},
};

static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

entry_point!(main, config = &BOOTLOADER_CONFIG);

fn main(boot_info: &'static mut BootInfo) -> ! {
    kernel::init(); // If you dare to call serial_print before this, I'm not responsible for the consequences...

    serial_println!("Hello World!");

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset.into_option().unwrap());
    let mut mapper = unsafe { kernel::mm::memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_regions) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("Heap initialization failed");

    let x = Box::new(42);

    kernel::drivers::exit::exit_qemu(kernel::drivers::exit::QemuExitCode::Success);
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);

    kernel::drivers::exit::exit_qemu(kernel::drivers::exit::QemuExitCode::Failed);
}
