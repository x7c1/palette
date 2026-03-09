use std::fmt;

use crate::task_id::TaskId;
use crate::task_status::TaskStatus;

/// Domain-level task errors.
#[derive(Debug)]
pub enum TaskError {
    NotFound {
        task_id: TaskId,
    },
    InvalidTransition {
        task_id: TaskId,
        from: TaskStatus,
        to: TaskStatus,
    },
    DuplicateId {
        task_id: TaskId,
    },
}

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskError::NotFound { task_id } => write!(f, "task not found: {task_id}"),
            TaskError::InvalidTransition { task_id, from, to } => {
                write!(
                    f,
                    "invalid transition for task {task_id}: {} -> {}",
                    from.as_str(),
                    to.as_str()
                )
            }
            TaskError::DuplicateId { task_id } => write!(f, "duplicate task id: {task_id}"),
        }
    }
}

impl std::error::Error for TaskError {}
