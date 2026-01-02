#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x11,
    Failed = 0x13,
}

/// Exit QEMU with the given exit code.
/// Note: The exit code must be odd to be correctly outputted by QEMU.
pub fn exit_qemu(exit_code: QemuExitCode) -> ! {
    use x86_64::instructions::{nop, port::Port};

    unsafe {
        let mut port = Port::new(0xf4);

        // So QEMU apparently does ((code << 1) | 1), so we need to shift it by 1 and our code must be odd
        port.write((exit_code as u32) >> 1);
    }

    loop {
        nop();
    }
}
