// Syscall handling
//
// Intizalizes the necessary syscall infrastructure and handles and distributes syscalls

use core::arch::naked_asm;

use raw_cpuid::CpuId;
use x86_64::{
    VirtAddr,
    registers::{
        control::{Cr0, Cr0Flags, Cr4, Cr4Flags, Efer, EferFlags},
        model_specific::{LStar, SFMask, Star},
        rflags::RFlags,
    },
};

use crate::{drivers::serial, gdt::GDT, serial_println};

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

        SFMask::write(RFlags::INTERRUPT_FLAG); // enable interrupts on syscall entry

        // Set up code segment selectors for syscall/sysret
        let cs_sysret = GDT.1.user_code; // RPL 3
        let ss_sysret = GDT.1.user_data; // RPL 3
        let cs_syscall = GDT.1.code; // RPL 0
        let ss_syscall = GDT.1.data; // RPL 0

        // For some stupid reason do we need to switch the order of cs and ss_sysret cuz we else get an error even tho the docs say otherwise
        // TODO: figure out why
        match Star::write(ss_sysret, cs_sysret, cs_syscall, ss_syscall) {
            Ok(_) => {}
            Err(e) => {
                serial_println!("Failed to write STAR register: {:?}", e);
                panic!("Failed to write STAR register");
            }
        }
    }
}

/// Syscall entry point - called when a syscall is invoked from user mode
/// This is a naked function that saves all registers, calls syscall_handler,
/// then restores registers and returns via iretq
#[unsafe(naked)]
extern "C" fn syscall_handler() {
    naked_asm!(
        // save context, see x86_64 ABI
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        // copy 4th argument to rcx to adhere x86_64 ABI
        "mov rcx, r10",
        "sti",
        // Call the actual syscall handler
        "call {syscall_entry}",
        // restore context, see x86_64 ABI
        "cli",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "sysretq",

        syscall_entry = sym syscall_entry,
    );
}

/// Actual syscall handler - called by syscall_handler after saving context
/// Arguments:
///     rdi: syscall number
///     rsi: first argument
///     rdx: second argument
///     rcx: third argument
///     r10: fourth argument
/// Returns:
///     rax: return value
extern "C" fn syscall_entry(rdi: u64, rsi: u64, rdx: u64, rcx: u64, r10: u64) -> u64 {
    serial_println!(
        "Syscall invoked: number={}, arg1={}, arg2={}, arg3={}, arg4={}",
        rdi,
        rsi,
        rdx,
        rcx,
        r10
    );

    // For now, just return 0
    0
}
