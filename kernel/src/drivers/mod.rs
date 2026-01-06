pub mod acpi;
pub mod apic;
pub mod exit;
pub mod keyboard;
pub mod mouse;
pub mod pit;
pub mod serial;

/// Initialize all drivers
pub fn init() {
    serial::init_serial();

    mouse::init_mouse();
}
