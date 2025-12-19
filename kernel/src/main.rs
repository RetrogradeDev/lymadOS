#![no_std]
#![no_main]

use core::panic::PanicInfo;

use bootloader_api::{BootInfo, entry_point};

use kernel::serial_println;

entry_point!(main);

fn main(_boot_info: &'static mut BootInfo) -> ! {
    kernel::drivers::serial::init_serial();

    serial_println!("Hello World!");

    kernel::drivers::exit::exit_qemu(kernel::drivers::exit::QemuExitCode::Success);
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);

    kernel::drivers::exit::exit_qemu(kernel::drivers::exit::QemuExitCode::Failed);
}
