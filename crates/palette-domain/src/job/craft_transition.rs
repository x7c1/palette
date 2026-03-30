use super::{CraftStatus, JobStatus, TransitionError};

/// Named transitions for Craft Job status.
///
/// Each variant encodes both the from→to states and the semantic meaning
/// of when this transition occurs. Reading this enum tells you the full
/// lifecycle of a Craft Job.
#[derive(Debug, Clone, Copy)]
pub enum CraftTransition {
    /// Todo → InProgress: Orchestrator assigns the Job to a Crafter.
    Start,
    /// InProgress → InReview: Crafter's session ends (stop hook).
    SubmitForReview,
    /// InReview → Done: All sibling Review Jobs are approved.
    Approve,
    /// InReview → InProgress: Review Integrator submits a ChangesRequested verdict.
    RequestChanges,
    /// Any → Escalated: Maximum review rounds exceeded.
    Escalate,
}

impl CraftTransition {
    pub fn from_status(self) -> Option<CraftStatus> {
        match self {
            CraftTransition::Start => Some(CraftStatus::Todo),
            CraftTransition::SubmitForReview => Some(CraftStatus::InProgress),
            CraftTransition::Approve => Some(CraftStatus::InReview),
            CraftTransition::RequestChanges => Some(CraftStatus::InReview),
            CraftTransition::Escalate => None, // any
        }
    }

    pub fn to_status(self) -> CraftStatus {
        match self {
            CraftTransition::Start => CraftStatus::InProgress,
            CraftTransition::SubmitForReview => CraftStatus::InReview,
            CraftTransition::Approve => CraftStatus::Done,
            CraftTransition::RequestChanges => CraftStatus::InProgress,
            CraftTransition::Escalate => CraftStatus::Escalated,
        }
    }

    pub fn to_job_status(self) -> JobStatus {
        JobStatus::Craft(self.to_status())
    }

    /// Validate that the current status matches the expected `from` status.
    pub fn validate(self, current: CraftStatus) -> Result<JobStatus, TransitionError> {
        if let Some(expected) = self.from_status()
            && current != expected
        {
            return Err(TransitionError {
                from: JobStatus::Craft(current),
                to: JobStatus::Craft(self.to_status()),
            });
        }
        Ok(self.to_job_status())
    }
}
