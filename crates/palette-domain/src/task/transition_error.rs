use std::fmt;

use super::{TaskStatus, TaskType};

/// Invalid status transition error.
#[derive(Debug)]
pub struct TransitionError {
    pub task_type: TaskType,
    pub from: TaskStatus,
    pub to: TaskStatus,
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid status transition for {} task: {} -> {}",
            self.task_type.as_str(),
            self.from.as_str(),
            self.to.as_str()
        )
    }
}

impl std::error::Error for TransitionError {}
