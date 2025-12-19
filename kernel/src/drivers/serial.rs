use spin::Mutex;
use uart_16550::SerialPort;

static SERIAL1: Mutex<Option<SerialPort>> = Mutex::new(None);

pub fn init_serial() {
    let mut serial_port = unsafe { SerialPort::new(0x3F8) };
    serial_port.init();
    *SERIAL1.lock() = Some(serial_port);
}

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    // Disable interrupts to prevent deadlocks or data corruption
    interrupts::without_interrupts(|| {
        SERIAL1
            .lock()
            .as_mut()
            .expect("Serial port not initialized")
            .write_fmt(args)
            .expect("Printing to serial failed");
    });
}

/// Prints to the host through the serial interface.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::drivers::serial::_print(format_args!($($arg)*))
    };
}

/// Prints to the host through the serial interface, appending a newline.
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*))
}
