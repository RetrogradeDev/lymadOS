use crate::{drivers::apic::end_interrupt, serial_println};
use x86_64::structures::idt::InterruptStackFrame;

pub extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // Handle keyboard input here
    serial_println!("Keyboard interrupt received");

    // Acknowledge the interrupt
    end_interrupt();
}
