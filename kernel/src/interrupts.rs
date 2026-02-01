use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

use crate::drivers;
use crate::tasks::switch::timer_interrupt_entry;
use crate::{
    drivers::exit::{QemuExitCode, exit_qemu},
    gdt, serial_println,
};

use spin::Lazy;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Keyboard = 33,
    Timer = 32,
    Mouse = 44,
}

pub static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();

    unsafe {
        idt.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);

        idt.general_protection_fault
            .set_handler_fn(general_protection_fault_handler)
            .set_stack_index(gdt::GENERAL_PROTECTION_FAULT_IST_INDEX);

        idt.page_fault
            .set_handler_fn(page_fault_handler)
            .set_stack_index(gdt::PAGE_FAULT_IST_INDEX);
    }

    idt.divide_error.set_handler_fn(divide_by_zero_handler);

    // We cast to the expected type since it's a naked function that manages its own frame
    unsafe {
        let handler: extern "x86-interrupt" fn(InterruptStackFrame) =
            core::mem::transmute(timer_interrupt_entry as *const ());
        idt[InterruptIndex::Timer as u8].set_handler_fn(handler);
    }

    idt[InterruptIndex::Keyboard as u8]
        .set_handler_fn(drivers::keyboard::keyboard_interrupt_handler);
    idt[InterruptIndex::Mouse as u8].set_handler_fn(drivers::mouse::mouse_interrupt_handler);

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

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    serial_println!("EXCEPTION: PAGE FAULT");
    serial_println!("Accessed Address: {:?}", Cr2::read());
    serial_println!("Error Code: {:?}", error_code);
    serial_println!("{:#?}", stack_frame);

    exit_qemu(QemuExitCode::Failed)
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    serial_println!("EXCEPTION: GENERAL PROTECTION FAULT");
    serial_println!("Error Code: {:?}", error_code);
    serial_println!("{:#?}", stack_frame);

    exit_qemu(QemuExitCode::Failed)
}
