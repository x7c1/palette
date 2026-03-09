use crate::models::{TaskId, TaskStatus, TaskType};
use std::fmt;

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

/// Domain-level review errors.
#[derive(Debug)]
pub enum ReviewError {
    TaskNotFound { review_task_id: TaskId },
    NotReviewTask { task_id: TaskId },
}

impl fmt::Display for ReviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReviewError::TaskNotFound { review_task_id } => {
                write!(f, "review task not found: {review_task_id}")
            }
            ReviewError::NotReviewTask { task_id } => {
                write!(f, "task {task_id} is not a review task")
            }
        }
    }
}

impl std::error::Error for ReviewError {}

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
