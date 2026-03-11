use std::fmt;

use super::{JobStatus, JobType};

/// Invalid status transition error.
#[derive(Debug)]
pub struct TransitionError {
    pub job_type: JobType,
    pub from: JobStatus,
    pub to: JobStatus,
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid status transition for {} job: {} -> {}",
            self.job_type.as_str(),
            self.from.as_str(),
            self.to.as_str()
        )
    }
}

impl std::error::Error for TransitionError {}
