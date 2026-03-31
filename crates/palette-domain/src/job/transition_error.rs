use std::fmt;

use super::JobStatus;

/// Status transition error.
#[derive(Debug, palette_macros::ReasonKey)]
pub enum TransitionError {
    Invalid { from: JobStatus, to: JobStatus },
}

impl TransitionError {
    pub fn invalid(from: impl Into<JobStatus>, to: impl Into<JobStatus>) -> Self {
        Self::Invalid {
            from: from.into(),
            to: to.into(),
        }
    }
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransitionError::Invalid { from, to } => {
                write!(f, "invalid status transition: {from} -> {to}")
            }
        }
    }
}

impl std::error::Error for TransitionError {}
