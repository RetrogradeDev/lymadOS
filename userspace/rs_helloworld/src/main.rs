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

        // Sleep for a bit to avoid spamming the output too much
        for _ in 0..1_000_000 {
            unsafe {
                core::arch::asm!("pause");
            }
        }
    }
}

fn print(s: &str) {
    unsafe {
        core::arch::asm!(
            "syscall",

            // inlateout tells the compiler that rax is both an input and an output register,
            // and that it will be overwritten by the syscall
            inlateout("rax") 1u64 => _,    // syscall number 1 = write
            in("rdi") 1u64,           // fd = 1 (stdout)
            in("rsi") s.as_ptr(),     // pointer to string
            in("rdx") s.len(),        // length of the string

            lateout("rcx") _,         // clobbered by syscall
            lateout("r11") _,         // clobbered by syscall

            options(nostack)
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
