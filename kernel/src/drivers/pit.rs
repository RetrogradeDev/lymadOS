use x86_64::structures::idt::InterruptStackFrame;

use crate::{drivers::apic::end_interrupt, serial_print};

pub extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    serial_print!(".");

    // Acknowledge the interrupt
    end_interrupt();
}
