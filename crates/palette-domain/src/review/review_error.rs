use std::fmt;

use crate::job::JobId;

/// Domain-level review errors.
#[derive(Debug)]
pub enum ReviewError {
    JobNotFound { review_job_id: JobId },
    NotReviewJob { job_id: JobId },
}

impl fmt::Display for ReviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReviewError::JobNotFound { review_job_id } => {
                write!(f, "review job not found: {review_job_id}")
            }
            ReviewError::NotReviewJob { job_id } => {
                write!(f, "job {job_id} is not a review job")
            }
        }
    }
}

impl std::error::Error for ReviewError {}
