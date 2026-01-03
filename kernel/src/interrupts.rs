use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use crate::{
    drivers::exit::{QemuExitCode, exit_qemu},
    serial_println,
};

use spin::Lazy;

pub static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();

    idt.double_fault.set_handler_fn(double_fault_handler);
    idt.divide_error.set_handler_fn(divide_by_zero_handler);

    idt
});

pub fn init() {
    IDT.load();
}

extern "x86-interrupt" fn divide_by_zero_handler(stack_frame: InterruptStackFrame) {
    serial_println!("EXCEPTION: DIVIDE BY ZERO\n{:#?}", stack_frame);

    exit_qemu(QemuExitCode::Failed)
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    serial_println!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);

    exit_qemu(QemuExitCode::Failed)
}
