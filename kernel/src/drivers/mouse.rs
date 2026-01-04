use crate::{drivers::apic::end_interrupt, serial_println};
use x86_64::structures::idt::InterruptStackFrame;

pub extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // Acknowledge the interrupt
    end_interrupt();

    // Handle mouse input here
    serial_println!("Mouse interrupt received");
}
