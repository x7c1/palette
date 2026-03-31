use super::{JobStatus, ReviewStatus, TransitionError};

/// Named transitions for Review Job status.
///
/// Each variant encodes both the from→to states and the semantic meaning
/// of when this transition occurs. Reading this enum tells you the full
/// lifecycle of a Review Job.
#[derive(Debug, Clone, Copy)]
pub enum ReviewTransition {
    /// Todo → InProgress: Orchestrator assigns the Job to a Reviewer.
    Start,
    /// InProgress → Done: Reviewer submits an Approved verdict.
    Approve,
    /// InProgress → ChangesRequested: Reviewer submits a ChangesRequested verdict.
    RequestChanges,
    /// ChangesRequested → InProgress: Crafter revised; re-review begins.
    Restart,
    /// Any → Escalated: Maximum review rounds exceeded.
    Escalate,
}

impl ReviewTransition {
    pub fn from_status(self) -> Option<ReviewStatus> {
        match self {
            ReviewTransition::Start => Some(ReviewStatus::Todo),
            ReviewTransition::Approve => Some(ReviewStatus::InProgress),
            ReviewTransition::RequestChanges => Some(ReviewStatus::InProgress),
            ReviewTransition::Restart => Some(ReviewStatus::ChangesRequested),
            ReviewTransition::Escalate => None, // any
        }
    }

    pub fn to_status(self) -> ReviewStatus {
        match self {
            ReviewTransition::Start => ReviewStatus::InProgress,
            ReviewTransition::Approve => ReviewStatus::Done,
            ReviewTransition::RequestChanges => ReviewStatus::ChangesRequested,
            ReviewTransition::Restart => ReviewStatus::InProgress,
            ReviewTransition::Escalate => ReviewStatus::Escalated,
        }
    }

    pub fn to_job_status(self) -> JobStatus {
        JobStatus::Review(self.to_status())
    }

    /// Validate that the current status matches the expected `from` status.
    pub fn validate(self, current: ReviewStatus) -> Result<JobStatus, TransitionError> {
        if let Some(expected) = self.from_status()
            && current != expected
        {
            return Err(TransitionError::invalid(
                JobStatus::Review(current),
                JobStatus::Review(self.to_status()),
            ));
        }
        Ok(self.to_job_status())
    }
}
