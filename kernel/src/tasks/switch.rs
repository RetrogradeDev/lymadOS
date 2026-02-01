// Context switch assembly routines
//
// This module provides the low-level assembly for saving and restoring
// CPU state during task switches.

use core::arch::asm;

use crate::drivers::apic::end_interrupt;
use crate::serial_print;
use crate::tasks::{SCHEDULER, TaskContext};

/// Pointer to where we should store the current RSP0 value for TSS updates
/// This is set by the GDT module to point to the TSS's RSP0 field
pub static mut TSS_RSP0_PTR: *mut u64 = core::ptr::null_mut();

/// Timer interrupt entry point - this is called from assembly
/// Saves current task state, potentially switches tasks, restores state
#[unsafe(no_mangle)]
pub extern "C" fn timer_tick(context_ptr: *mut TaskContext) {
    let context = unsafe { &mut *context_ptr };

    // Check if we came from user mode
    let from_usermode = (context.cs & 3) == 3;

    // Get scheduler
    let mut scheduler = SCHEDULER.lock();

    if !scheduler.is_initialized() {
        // Scheduler not ready yet, just print and return
        // This shouldn't happen, but just in case
        if from_usermode {
            serial_print!("u");
        } else {
            serial_print!(".");
        }
        drop(scheduler);
        end_interrupt();
        return;
    }

    // Print task ID
    if let Some(task_id) = scheduler.current_task_id() {
        serial_print!("{}", task_id);
    }

    // Try to schedule next task
    if let Some((old_ctx, new_ctx, new_kernel_stack)) = scheduler.schedule() {
        // Copy the current context to the old task
        unsafe {
            *old_ctx = *context;
        }

        // Load the new task's context
        unsafe {
            *context = *new_ctx;

            // Update TSS RSP0 to point to the new task's kernel stack
            if !TSS_RSP0_PTR.is_null() {
                *TSS_RSP0_PTR = new_kernel_stack;
            }
        }
    }

    drop(scheduler); // prevent deadlock

    // Acknowledge interrupt
    end_interrupt();
}

/// The actual timer interrupt handler entry point
/// This is a naked function that saves all registers, calls timer_tick,
/// then restores registers and returns via iretq
#[unsafe(naked)]
pub extern "C" fn timer_interrupt_entry() {
    core::arch::naked_asm!(
        // At this point the CPU has pushed: SS, RSP, RFLAGS, CS, RIP
        // We need to save all general-purpose registers

        // Push all GP registers (in reverse order of TaskContext struct)
        "push rax",
        "push rbx",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push rbp",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        // RSP now points to TaskContext on the stack
        // Pass it as argument to timer_tick
        "mov rdi, rsp",
        "call timer_tick",
        // Restore all GP registers (TaskContext may have been modified!)
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rbp",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rbx",
        "pop rax",
        // Return from interrupt - CPU will pop RIP, CS, RFLAGS, RSP, SS
        "iretq",
    );
}

/// Switch to the first task (initial entry into user mode)
/// This sets up the context and jumps to user mode
///
/// TODO: Save FPU/MMX/SSE state if needed
#[unsafe(no_mangle)]
pub unsafe fn switch_to_first_task() -> ! {
    let scheduler = SCHEDULER.lock();

    let context = scheduler.current_context().expect("No tasks to run");
    let kernel_stack = scheduler
        .current_kernel_stack_top()
        .expect("No kernel stack");

    // Update TSS RSP0
    unsafe {
        if !TSS_RSP0_PTR.is_null() {
            *TSS_RSP0_PTR = kernel_stack;
        }
    }

    // Load the context values
    let rip = context.rip;
    let cs = context.cs;
    let rflags = context.rflags;
    let rsp = context.rsp;
    let ss = context.ss;

    // Get user data selector for segment registers
    let user_data = ss;

    drop(scheduler);

    // Set up segments and iretq to user mode
    unsafe {
        asm!(
            // Set data segments to user data selector
            "mov ds, {ds:x}",
            "mov es, {ds:x}",
            "mov fs, {ds:x}",
            "mov gs, {ds:x}",

            // Push iretq frame
            "push {ss}",
            "push {rsp}",
            "push {rflags}",
            "push {cs}",
            "push {rip}",

            // Jump to user mode
            "iretq",

            ds = in(reg) user_data,
            ss = in(reg) ss,
            rsp = in(reg) rsp,
            rflags = in(reg) rflags,
            cs = in(reg) cs,
            rip = in(reg) rip,
            options(noreturn)
        );
    }
}
