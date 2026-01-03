#![no_std]
#![feature(abi_x86_interrupt)]

extern crate alloc;

use x86_64::instructions::hlt;

pub mod drivers;
pub mod interrupts;
pub mod mm;

/// Initialize the kernel
pub fn init() {
    drivers::init();

    interrupts::init();
}

/// Halt the CPU forever
pub fn hlt_loop() -> ! {
    loop {
        hlt();
    }
}
