//! Types related to task management

use super::TaskContext;

/// The task control block (TCB) of a task.
#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    /// The task status in it's lifecycle
    pub task_status: TaskStatus,
    /// The task context
    pub task_cx: TaskContext,
    /// The count of each syscall invoked by this task
    pub syscall_count: [usize; crate::syscall::SYSCALL_NUM],
}

impl TaskControlBlock {
    /// Create a new `TaskControlBlock` with blanck values
    pub fn new() -> Self {
        Self {
            task_cx: TaskContext::zero_init(),
            task_status: TaskStatus::UnInit,
            syscall_count: [0; crate::syscall::SYSCALL_NUM],
        }
    }
}

/// The status of a task
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    /// uninitialized
    UnInit,
    /// ready to run
    Ready,
    /// running
    Running,
    /// exited
    Exited,
}
