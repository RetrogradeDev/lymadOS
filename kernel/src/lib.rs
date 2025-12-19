#![no_std]
#![feature(abi_x86_interrupt)]

use x86_64::instructions::hlt;

pub mod drivers;

/// Halt the CPU forever
pub fn hlt_loop() -> ! {
    loop {
        hlt();
    }
}
