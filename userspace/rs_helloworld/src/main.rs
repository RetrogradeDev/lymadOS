#![no_std]
#![no_main]

use core::arch::global_asm;

#[unsafe(no_mangle)]
fn main() -> ! {
    print("Test");
    print("More test");
    print("Test");

    loop {
        print("Hello, world!");

        for _ in 0..1_000_000_000 {
            // Just a busy loop to slow down the printing
        }
    }
}

fn print(s: &str) {
    unsafe {
        core::arch::asm!(
            "mov rax, 1", // syscall number 1 = print
            "mov rdi, 1", // fd = 1 (stdout)
            "mov rsi, {arg}",    // value to print
            "syscall",
            arg = in(reg) s.as_ptr() as u64,
        );
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

// Start our program
global_asm!(
    r#".global _start
    _start:
    call main
"#
);
