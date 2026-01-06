use crate::{
    drivers::apic::end_interrupt,
    events::{Event, push_event},
};
use ps2_mouse::{Mouse, MouseState};
use x86_64::{instructions::port::PortReadOnly, structures::idt::InterruptStackFrame};

static mut MOUSE: Mouse = Mouse::new();

pub extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut port = PortReadOnly::new(0x60);
    let data: u8 = unsafe { port.read() };

    #[allow(static_mut_refs)]
    unsafe {
        MOUSE.process_packet(data)
    };

    // Acknowledge the interrupt
    end_interrupt();
}

pub fn init_mouse() {
    #[allow(static_mut_refs)] // Who cares about safety anyway hehehe
    {
        unsafe {
            MOUSE.set_on_complete(handle_on_complete);
            MOUSE.init().unwrap();
        };
    }
}

fn handle_on_complete(state: MouseState) {
    push_event(Event::MouseEvent(state));
}
