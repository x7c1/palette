use std::fmt;

use super::{JobId, JobStatus};

/// Domain-level job errors.
#[derive(Debug)]
pub enum JobError {
    NotFound {
        job_id: JobId,
    },
    InvalidTransition {
        job_id: JobId,
        from: JobStatus,
        to: JobStatus,
    },
    DuplicateId {
        job_id: JobId,
    },
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JobError::NotFound { job_id } => write!(f, "job not found: {job_id}"),
            JobError::InvalidTransition { job_id, from, to } => {
                write!(f, "invalid transition for job {job_id}: {from} -> {to}")
            }
            JobError::DuplicateId { job_id } => write!(f, "duplicate job id: {job_id}"),
        }
    }
}

impl std::error::Error for JobError {}
