// Syscall handling
//
// Initializes the necessary syscall infrastructure and handles and distributes syscalls

use core::arch::naked_asm;

use raw_cpuid::CpuId;
use x86_64::{
    VirtAddr,
    registers::{
        control::{Cr0, Cr0Flags, Cr4, Cr4Flags, Efer, EferFlags},
        model_specific::{LStar, Msr, SFMask},
        rflags::RFlags,
    },
};

use crate::{gdt::GDT, serial_println};

const SYSCALL_STACK_SIZE: usize = 4096 * 4; // 16 KiB

/// Kernel stack for syscall handler
/// We need a dedicated stack because syscall does NOT switch RSP automatically
#[repr(C, align(16))]
struct SyscallStack([u8; SYSCALL_STACK_SIZE]);

#[unsafe(no_mangle)]
static mut SYSCALL_KERNEL_STACK: SyscallStack = SyscallStack([0; SYSCALL_STACK_SIZE]);

pub fn init_syscalls() {
    // First, enable the necessary CPU features for syscalls
    unsafe {
        Cr0::update(|cr0| {
            *cr0 |= Cr0Flags::ALIGNMENT_MASK;
            *cr0 |= Cr0Flags::NUMERIC_ERROR;
            *cr0 |= Cr0Flags::MONITOR_COPROCESSOR;
            // enable cache
            *cr0 &= !(Cr0Flags::CACHE_DISABLE | Cr0Flags::NOT_WRITE_THROUGH);
        });

        let cpuid = CpuId::new();

        Cr4::update(|cr4| {
            // disable performance monitoring counter
            // allow the usage of rdtsc in user space
            *cr4 &= !(Cr4Flags::PERFORMANCE_MONITOR_COUNTER | Cr4Flags::TIMESTAMP_DISABLE);

            let has_pge = match cpuid.get_feature_info() {
                Some(finfo) => finfo.has_pge(),
                None => false,
            };

            if has_pge {
                *cr4 |= Cr4Flags::PAGE_GLOBAL; // enable global pages
            }

            let has_fsgsbase = match cpuid.get_extended_feature_info() {
                Some(efinfo) => efinfo.has_fsgsbase(),
                None => false,
            };

            if has_fsgsbase {
                *cr4 |= Cr4Flags::FSGSBASE;
            }

            let has_mce = match cpuid.get_feature_info() {
                Some(finfo) => finfo.has_mce(),
                None => false,
            };

            if has_mce {
                *cr4 |= Cr4Flags::MACHINE_CHECK_EXCEPTION; // enable machine check exceptions
            }
        });
    };

    // Next, initialize all registers and write the syscall entry point
    unsafe {
        Efer::update(|efer| {
            *efer |= EferFlags::SYSTEM_CALL_EXTENSIONS; // enable syscall/sysret instructions
        });

        LStar::write(VirtAddr::new(syscall_handler as u64)); // set syscall entry point

        SFMask::write(RFlags::INTERRUPT_FLAG); // mask interrupts on syscall entry (clear IF)

        // Set up code segment selectors for syscall/sysret using raw MSR write cuz the Star::write acts weird
        // STAR MSR (0xC0000081) layout:
        //   Bits 31:0  = Reserved (should be 0)
        //   Bits 47:32 = SYSCALL CS (kernel code segment, SS = CS+8)
        //   Bits 63:48 = SYSRET base (64-bit CS = base+16, SS = base+8)
        //
        // Our GDT layout:
        //   0x08 = kernel_code
        //   0x10 = kernel_data
        //   0x18 = user_data (0x1B with RPL=3)
        //   0x20 = user_code (0x23 with RPL=3)
        //
        // For SYSCALL: CS=0x08, SS=0x10 (kernel_data at CS+8)
        // For SYSRET 64-bit: base=0x10, so CS=0x10+16=0x20, SS=0x10+8=0x18
        //
        // STAR value = (sysret_base << 48) | (syscall_cs << 32)
        //            = (0x10 << 48) | (0x08 << 32)

        let syscall_cs: u64 = GDT.1.code.0 as u64; // 0x08
        let sysret_base: u64 = (GDT.1.user_data.0 & !3) as u64 - 8; // 0x18 - 8 = 0x10

        serial_println!("Setting up STAR register:");
        serial_println!("  Kernel CS (SYSCALL): {:#x}", syscall_cs);
        serial_println!("  SYSRET base: {:#x}", sysret_base);
        serial_println!("  SYSRET CS will be: {:#x}", sysret_base + 16);
        serial_println!("  SYSRET SS will be: {:#x}", sysret_base + 8);

        let star_value = (sysret_base << 48) | (syscall_cs << 32);
        serial_println!("  STAR value: {:#x}", star_value);

        // Write to STAR MSR (0xC0000081)
        const STAR_MSR: u32 = 0xC0000081;
        let mut star_msr = Msr::new(STAR_MSR);
        star_msr.write(star_value);

        serial_println!(
            "Syscall initialized, handler at {:#x}",
            syscall_handler as u64
        );
    }
}

/// Syscall entry point - called when a syscall is invoked from user mode
///
/// On entry (from syscall instruction):
///   RCX = return RIP (user's next instruction)
///   R11 = saved RFLAGS
///   RSP = user stack (NOT changed by syscall!)
///   
/// We must:
///   1. Switch to kernel stack
///   2. Save user RSP
///   3. Save registers
///   4. Call the actual handler
///   5. Restore everything and sysretq
#[unsafe(naked)]
extern "C" fn syscall_handler() {
    naked_asm!(
        // At this point we're on the USER stack - dangerous!
        // RCX = return address, R11 = saved rflags

        // Save user RSP in a scratch register
        "mov r10, rsp",

        // Load kernel stack using RIP-relative addressing for PIE compatibility
        "lea rsp, [rip + {kernel_stack} + {stack_size}]",

        // Now we're on kernel stack - safe to push
        // Save the user stack pointer
        "push r10",

        // Save RCX (return RIP) and R11 (saved RFLAGS) - critical for sysretq!
        "push rcx",
        "push r11",

        // Save caller-saved registers per System V ABI
        "push rax",
        "push rdi",
        "push rsi",
        "push rdx",
        "push r8",
        "push r9",

        // Enable interrupts now that we're on a safe stack
        "sti",

        // Call the actual syscall handler
        // Syscall number is in rax, pass as first arg
        "mov rdi, rax",
        "lea rax, [rip + {syscall_entry}]",
        "call rax",

        // Return value is in RAX - leave it there

        // Disable interrupts for sysret
        "cli",

        // Restore caller-saved registers (skip rax - it has return value)
        "pop r9",
        "pop r8",
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "add rsp, 8",       // skip saved rax

        // Restore R11 (rflags) and RCX (return address)
        "pop r11",
        "pop rcx",

        // Restore user RSP
        "pop rsp",

        // Return to user mode
        "sysretq",

        kernel_stack = sym SYSCALL_KERNEL_STACK,
        stack_size = const SYSCALL_STACK_SIZE,
        syscall_entry = sym syscall_entry,
    );
}

/// Actual syscall handler - called by syscall_handler after saving context
/// Arguments:
///     rdi: syscall number (was in rax)
/// Returns:
///     rax: return value
extern "C" fn syscall_entry(syscall_num: u64) -> u64 {
    serial_println!("Syscall invoked: number={}", syscall_num);

    // For now, just return 0
    0
}
