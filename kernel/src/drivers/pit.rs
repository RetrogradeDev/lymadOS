use x86_64::structures::idt::InterruptStackFrame;

use crate::{drivers::apic::end_interrupt, serial_print};

pub extern "x86-interrupt" fn timer_interrupt_handler(stack_frame: InterruptStackFrame) {
    // Check if we came from user mode (Ring 3) by looking at the code segment's RPL
    let cs = stack_frame.code_segment.0;
    let from_usermode = (cs & 3) == 3;

    if from_usermode {
        serial_print!("u"); // 'u' for user mode tick
    } else {
        serial_print!("."); // '.' for kernel mode tick
    }

    // Acknowledge the interrupt
    end_interrupt();
}
