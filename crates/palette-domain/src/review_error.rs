use std::fmt;

use crate::task_id::TaskId;

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
