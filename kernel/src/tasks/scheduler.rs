use crate::tasks::task::{Task, TaskContext, TaskState};
use alloc::vec::Vec;

/// Simple round-robin scheduler
// TODO: More advanced scheduling algorithms, task sleeping/waking, inter-task communication, etc.
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
