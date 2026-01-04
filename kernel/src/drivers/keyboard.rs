use crate::{drivers::apic::end_interrupt, serial_println};
use pc_keyboard::{HandleControl, Keyboard, ScancodeSet1, layouts};
use spin::{Lazy, Mutex};
use x86_64::instructions::port::Port;
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
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    let mut keyboard = KEYBOARD.lock();
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                pc_keyboard::DecodedKey::Unicode(character) => {
                    serial_println!("Key pressed: {}", character);
                }
                pc_keyboard::DecodedKey::RawKey(key) => {
                    serial_println!("Key pressed: {:?}", key);
                }
            }
        }
    }

    // Acknowledge the interrupt
    end_interrupt();
}
