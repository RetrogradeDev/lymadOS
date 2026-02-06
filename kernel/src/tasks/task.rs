use crate::{
    mm::user::set_page_user_accessible,
    serial_println,
    tasks::{KERNEL_STACK_SIZE, USER_STACK_SIZE, elf},
};
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

    /// User-mode stack (heap allocated)
    _user_stack: Box<[u8; USER_STACK_SIZE]>,
    /// Kernel-mode stack for this task (used when handling interrupts from this task)
    pub kernel_stack: Box<[u8; KERNEL_STACK_SIZE]>,
    /// Code page (heap allocated, marked user-accessible)
    _code_page: Box<[u8; 4096]>,
}

impl Task {
    /// Create a new task with the given entry point code
    ///
    /// # Safety
    /// The caller must ensure the mapper is valid and the code will be copied
    /// to a user-accessible page.
    pub unsafe fn new(
        entry_code: &[u8],
        mapper: &mut x86_64::structures::paging::OffsetPageTable,
    ) -> Self {
        use x86_64::VirtAddr;
        use x86_64::structures::paging::{Page, Size4KiB};

        let id = NEXT_TASK_ID.fetch_add(1, Ordering::SeqCst);

        // Allocate user stack
        let user_stack = Box::new([0u8; USER_STACK_SIZE]);
        let user_stack_top = user_stack.as_ptr() as u64 + USER_STACK_SIZE as u64;

        // Allocate kernel stack for this task
        let kernel_stack = Box::new([0u8; KERNEL_STACK_SIZE]);

        // Allocate code page and copy the entry code
        let mut code_page = Box::new([0u8; 4096]);
        let copy_len = entry_code.len().min(4096);
        code_page[..copy_len].copy_from_slice(&entry_code[..copy_len]);
        let code_ptr = code_page.as_ptr() as u64;

        // Mark user stack as user-accessible
        let stack_page: Page<Size4KiB> =
            Page::containing_address(VirtAddr::new(user_stack.as_ptr() as u64));
        unsafe {
            set_page_user_accessible(mapper, stack_page, true, false);
        }

        // Mark code page as user-accessible and executable
        let code_page_addr: Page<Size4KiB> = Page::containing_address(VirtAddr::new(code_ptr));
        unsafe {
            set_page_user_accessible(mapper, code_page_addr, false, true);
        }

        // Create context pointing to user code and stack
        let context = TaskContext::new_user(code_ptr, user_stack_top);

        Task {
            id,
            state: TaskState::Ready,
            context,
            _user_stack: user_stack,
            kernel_stack,
            _code_page: code_page,
        }
    }

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
        serial_println!("from_elf: starting, elf_data len={}", elf_data.len());
        let id = NEXT_TASK_ID.fetch_add(1, Ordering::SeqCst);
        serial_println!("from_elf: id={}", id);

        // Load ELF and allocate user stack
        serial_println!("from_elf: calling load_elf...");
        let elf::ElfLoadResult {
            entry_point,
            stack_top,
        } = elf::load_elf(elf_data, mapper, frame_allocator, phys_mem_offset)?;

        serial_println!(
            "from_elf: load_elf returned entry=0x{:x}, stack=0x{:x}",
            entry_point,
            stack_top
        );

        // Allocate kernel stack for this task (used during interrupts)
        // TODO: Consider something better
        let kernel_stack = Box::new([0u8; KERNEL_STACK_SIZE]);

        // We don't need the heap-based user stack or code page for ELF tasks
        // since they're mapped at fixed virtual addresses.
        // But we need placeholders to satisfy the struct.
        // TODO: Fix this
        let user_stack = Box::new([0u8; USER_STACK_SIZE]);
        let code_page = Box::new([0u8; 4096]);

        // Create context with ELF entry point and mapped stack
        let context = TaskContext::new_user(entry_point, stack_top);

        Ok(Task {
            id,
            state: TaskState::Ready,
            context,
            _user_stack: user_stack,
            kernel_stack,
            _code_page: code_page,
        })
    }

    /// Get the top of this task's kernel stack
    pub fn kernel_stack_top(&self) -> u64 {
        self.kernel_stack.as_ptr() as u64 + KERNEL_STACK_SIZE as u64
    }
}
