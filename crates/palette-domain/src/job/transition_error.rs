use std::fmt;

use super::JobStatus;

/// Invalid status transition error.
#[derive(Debug)]
pub struct TransitionError {
    pub from: JobStatus,
    pub to: JobStatus,
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid status transition: {} -> {}", self.from, self.to)
    }
}

impl std::error::Error for TransitionError {}

impl palette_core::ReasonKey for TransitionError {
    fn namespace(&self) -> &str {
        "job_status"
    }

    fn value(&self) -> &str {
        "invalid_transition"
    }
}
