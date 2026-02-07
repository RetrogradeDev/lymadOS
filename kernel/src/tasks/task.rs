use crate::tasks::{KERNEL_STACK_SIZE, elf};
use alloc::boxed::Box;
use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, OffsetPageTable, Size4KiB},
};

use crate::gdt::GDT;

/// Counter for generating unique task IDs
static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

/// CPU register state saved during context switch
/// This struct is used by the assembly context switch code
/// Layout must match the push/pop order in switch.rs
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TaskContext {
    // General purpose registers (saved/restored by our code)
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,

    // Interrupt stack frame (pushed by CPU on interrupt)
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

impl TaskContext {
    /// Create a new context for a user-mode task
    pub fn new_user(entry_point: u64, user_stack_top: u64) -> Self {
        let user_code = (GDT.1.user_code.0 | 3) as u64;
        let user_data = (GDT.1.user_data.0 | 3) as u64;

        Self {
            // General purpose registers start at 0
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            r11: 0,
            r10: 0,
            r9: 0,
            r8: 0,
            rbp: 0,
            rdi: 0,
            rsi: 0,
            rdx: 0,
            rcx: 0,
            rbx: 0,
            rax: 0,

            // Interrupt frame for iretq
            rip: entry_point,
            cs: user_code,
            rflags: 0x200, // IF (Interrupt Flag) enabled
            rsp: user_stack_top,
            ss: user_data,
        }
    }
}

/// Task state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Blocked,
}

/// A single task/process
pub struct Task {
    pub id: u64,
    pub state: TaskState,
    pub context: TaskContext,

    /// Kernel-mode stack for this task (used when handling interrupts from this task)
    pub kernel_stack: Box<[u8; KERNEL_STACK_SIZE]>,
}

impl Task {
    /// Create a new task from an ELF binary
    ///
    /// Loads the ELF into memory at its specified virtual addresses,
    /// allocates a user stack, and creates the task context.
    pub unsafe fn from_elf(
        elf_data: &[u8],
        mapper: &mut OffsetPageTable,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        phys_mem_offset: VirtAddr,
    ) -> Result<Self, elf::Error> {
        let id = NEXT_TASK_ID.fetch_add(1, Ordering::SeqCst);

        // Load ELF and allocate user stack
        let elf::ElfLoadResult {
            entry_point,
            stack_top,
        } = elf::load_elf(elf_data, mapper, frame_allocator, phys_mem_offset)?;

        // Allocate kernel stack for this task (used during interrupts)
        // TODO: Consider something better
        let kernel_stack = Box::new([0u8; KERNEL_STACK_SIZE]);

        // Create context with ELF entry point and mapped stack
        let context = TaskContext::new_user(entry_point, stack_top);

        Ok(Task {
            id,
            state: TaskState::Ready,
            context,
            kernel_stack,
        })
    }

    /// Get the top of this task's kernel stack
    pub fn kernel_stack_top(&self) -> u64 {
        self.kernel_stack.as_ptr() as u64 + KERNEL_STACK_SIZE as u64
    }
}
