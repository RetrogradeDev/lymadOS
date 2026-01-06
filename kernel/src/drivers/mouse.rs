use crate::{drivers::apic::end_interrupt, serial_println};
use x86_64::{instructions::port::PortReadOnly, structures::idt::InterruptStackFrame};

pub extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut port = PortReadOnly::new(0x60);
    let data: u8 = unsafe { port.read() };

    // Handle mouse input here
    serial_println!("Mouse interrupt received: data = {}", data);

    // Acknowledge the interrupt
    end_interrupt();
}
