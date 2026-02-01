// Task management and scheduling
//
// This module provides basic preemptive multitasking support with:
// - TaskContext: CPU register state for context switching
// - Task: Individual task/process representation
// - Scheduler: Round-robin task scheduling // TODO: More advanced scheduling algorithms

// TODO: Split into multiple files if it gets too big

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

use crate::gdt::GDT;

pub mod switch;
pub mod syscall;

/// Size of each task's user stack (1 page = 4KiB)
const USER_STACK_SIZE: usize = 4096; // TODO: Auto scale or smth

/// Size of each task's kernel stack (1 page = 4KiB)  
const KERNEL_STACK_SIZE: usize = 4096;

pub fn init() {
    syscall::init_syscalls();
}

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

/// Counter for generating unique task IDs
static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

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

    /// Get the top of this task's kernel stack
    pub fn kernel_stack_top(&self) -> u64 {
        self.kernel_stack.as_ptr() as u64 + KERNEL_STACK_SIZE as u64
    }
}

/// Global scheduler instance
pub static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());

/// Simple round-robin scheduler
pub struct Scheduler {
    tasks: Vec<Task>,
    current: usize,
    initialized: bool,
}

impl Scheduler {
    pub const fn new() -> Self {
        Self {
            tasks: Vec::new(),
            current: 0,
            initialized: false,
        }
    }

    /// Add a task to the scheduler
    pub fn add_task(&mut self, task: Task) {
        self.tasks.push(task);
    }

    /// Get the number of tasks
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Mark scheduler as initialized and set first task as running
    pub fn start(&mut self) {
        if !self.tasks.is_empty() {
            self.tasks[0].state = TaskState::Running;
            self.initialized = true;
        }
    }

    /// Check if scheduler is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get the current task's context (for initial switch)
    pub fn current_context(&self) -> Option<&TaskContext> {
        if self.tasks.is_empty() {
            None
        } else {
            Some(&self.tasks[self.current].context)
        }
    }

    /// Get mutable reference to current task's context
    pub fn current_context_mut(&mut self) -> Option<&mut TaskContext> {
        if self.tasks.is_empty() {
            None
        } else {
            Some(&mut self.tasks[self.current].context)
        }
    }

    /// Get current task's kernel stack top (for TSS RSP0)
    pub fn current_kernel_stack_top(&self) -> Option<u64> {
        if self.tasks.is_empty() {
            None
        } else {
            Some(self.tasks[self.current].kernel_stack_top())
        }
    }

    /// Get current task ID
    pub fn current_task_id(&self) -> Option<u64> {
        if self.tasks.is_empty() {
            None
        } else {
            Some(self.tasks[self.current].id)
        }
    }

    /// Schedule the next task (round-robin)
    /// Returns (old_context_ptr, new_context_ptr, new_kernel_stack_top)
    pub fn schedule(&mut self) -> Option<(*mut TaskContext, *const TaskContext, u64)> {
        if self.tasks.len() < 2 {
            return None; // Nothing to switch to
        }

        // Save current task as Ready
        self.tasks[self.current].state = TaskState::Ready;
        let old_context = &mut self.tasks[self.current].context as *mut TaskContext;

        // Move to next task (round-robin)
        self.current = (self.current + 1) % self.tasks.len();

        // Mark new task as Running
        self.tasks[self.current].state = TaskState::Running;
        let new_context = &self.tasks[self.current].context as *const TaskContext;
        let new_kernel_stack = self.tasks[self.current].kernel_stack_top();

        Some((old_context, new_context, new_kernel_stack))
    }
}

/// Set USER_ACCESSIBLE flag on all page table levels for a given page
/// probably the ugliest and most inefficient code ever but if it works, don't touch it
/// TODO: should be moved to mm module eventually but im lazy
unsafe fn set_page_user_accessible(
    mapper: &mut x86_64::structures::paging::OffsetPageTable,
    page: x86_64::structures::paging::Page<x86_64::structures::paging::Size4KiB>,
    writable: bool,
    executable: bool,
) {
    use x86_64::registers::control::Cr3;
    use x86_64::structures::paging::{PageTable, PageTableFlags};

    let virt = page.start_address();
    let phys_offset = mapper.phys_offset();

    let (l4_frame, _) = Cr3::read();
    let l4_table: &mut PageTable =
        unsafe { &mut *(phys_offset + l4_frame.start_address().as_u64()).as_mut_ptr() };

    let l4_entry = &mut l4_table[virt.p4_index()];
    l4_entry.set_flags(l4_entry.flags() | PageTableFlags::USER_ACCESSIBLE);

    let l3_frame = l4_entry.frame().expect("L4 entry not present");
    let l3_table: &mut PageTable =
        unsafe { &mut *(phys_offset + l3_frame.start_address().as_u64()).as_mut_ptr() };
    let l3_entry = &mut l3_table[virt.p3_index()];
    l3_entry.set_flags(l3_entry.flags() | PageTableFlags::USER_ACCESSIBLE);

    let l2_frame = l3_entry.frame().expect("L3 entry not present");
    let l2_table: &mut PageTable =
        unsafe { &mut *(phys_offset + l2_frame.start_address().as_u64()).as_mut_ptr() };
    let l2_entry = &mut l2_table[virt.p2_index()];

    if l2_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
        let mut new_flags = l2_entry.flags() | PageTableFlags::USER_ACCESSIBLE;
        if writable {
            new_flags |= PageTableFlags::WRITABLE;
        }
        if executable {
            new_flags &= !PageTableFlags::NO_EXECUTE;
        }
        l2_entry.set_flags(new_flags);
    } else {
        l2_entry.set_flags(l2_entry.flags() | PageTableFlags::USER_ACCESSIBLE);

        let l1_frame = l2_entry.frame().expect("L2 entry not present");
        let l1_table: &mut PageTable =
            unsafe { &mut *(phys_offset + l1_frame.start_address().as_u64()).as_mut_ptr() };
        let l1_entry = &mut l1_table[virt.p1_index()];

        let mut new_flags = l1_entry.flags() | PageTableFlags::USER_ACCESSIBLE;
        if writable {
            new_flags |= PageTableFlags::WRITABLE;
        }
        if executable {
            new_flags &= !PageTableFlags::NO_EXECUTE;
        }
        l1_entry.set_flags(new_flags);
    }

    x86_64::instructions::tlb::flush(virt);
}
