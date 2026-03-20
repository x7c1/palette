use crate::task::{TaskId, TaskStatus};

/// Side effects produced by the task rule engine after a task state transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskEffect {
    /// A task's status was changed by the rule engine.
    TaskStatusChanged {
        task_id: TaskId,
        new_status: TaskStatus,
    },
    /// A leaf task with a job type is ready to execute.
    TaskReady { task_id: TaskId },
}
