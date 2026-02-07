// Task management and scheduling
//
// This module provides basic preemptive multitasking support with:
// - TaskContext: CPU register state for context switching
// - Task: Individual task/process representation
// - Scheduler: Round-robin task scheduling

use spin::Mutex;

use crate::tasks::scheduler::Scheduler;

pub mod elf;
pub mod scheduler;
pub mod switch;
pub mod syscall;
pub mod task;

/// Size of each task's kernel stack (1 page = 4KiB)  
const KERNEL_STACK_SIZE: usize = 4096;

/// Global scheduler instance
pub static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());

pub fn init() {
    syscall::init_syscalls();
}
