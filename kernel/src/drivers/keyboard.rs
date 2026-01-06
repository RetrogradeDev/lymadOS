use crate::drivers::apic::end_interrupt;
use crate::events::{Event, KeyboardEvent, push_event};
use pc_keyboard::{HandleControl, Keyboard, ScancodeSet1, layouts};
use spin::{Lazy, Mutex};
use x86_64::instructions::port::PortReadOnly;
use x86_64::structures::idt::InterruptStackFrame;

// TODO: Do some research on scancode sets
static KEYBOARD: Lazy<Mutex<Keyboard<layouts::Azerty, ScancodeSet1>>> = Lazy::new(|| {
    Mutex::new(Keyboard::new(
        ScancodeSet1::new(),
        layouts::Azerty,
        HandleControl::Ignore,
    ))
});

pub extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut port = PortReadOnly::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    let mut keyboard = KEYBOARD.lock();
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        let event = match key_event.state {
            pc_keyboard::KeyState::Down => KeyboardEvent::KeyPressed(key_event.code),
            pc_keyboard::KeyState::Up => KeyboardEvent::KeyReleased(key_event.code),
            pc_keyboard::KeyState::SingleShot => KeyboardEvent::SingleShot(key_event.code),
        };

        push_event(Event::KeyboardEvent(event));
    }

    // Acknowledge the interrupt
    end_interrupt();
}
