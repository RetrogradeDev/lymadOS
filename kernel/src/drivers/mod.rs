pub mod exit;
pub mod serial;

/// Initialize all drivers
pub fn init() {
    serial::init_serial();
}
