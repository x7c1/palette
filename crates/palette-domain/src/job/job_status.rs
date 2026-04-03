use std::fmt;

use super::JobType;
use super::craft_status::CraftStatus;
use super::mechanized_status::MechanizedStatus;
use super::review_status::ReviewStatus;

/// Typed job status that pairs with the job type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Craft(CraftStatus),
    Review(ReviewStatus),
    Orchestrator(MechanizedStatus),
    Operator(MechanizedStatus),
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Craft(s) => s.as_str(),
            JobStatus::Review(s) => s.as_str(),
            JobStatus::Orchestrator(s) | JobStatus::Operator(s) => s.as_str(),
        }
    }

    /// Create a Todo status for the given job type.
    pub fn todo(job_type: JobType) -> Self {
        match job_type {
            JobType::Craft => JobStatus::Craft(CraftStatus::Todo),
            JobType::Review | JobType::ReviewIntegrate => JobStatus::Review(ReviewStatus::Todo),
            JobType::Orchestrator => JobStatus::Orchestrator(MechanizedStatus::Todo),
            JobType::Operator => JobStatus::Operator(MechanizedStatus::Todo),
        }
    }

    /// Create an InProgress status for the given job type.
    pub fn in_progress(job_type: JobType) -> Self {
        match job_type {
            JobType::Craft => JobStatus::Craft(CraftStatus::InProgress),
            JobType::Review | JobType::ReviewIntegrate => {
                JobStatus::Review(ReviewStatus::InProgress)
            }
            JobType::Orchestrator => JobStatus::Orchestrator(MechanizedStatus::InProgress),
            JobType::Operator => JobStatus::Operator(MechanizedStatus::InProgress),
        }
    }

    /// Returns true if the job is done.
    pub fn is_done(&self) -> bool {
        matches!(
            self,
            JobStatus::Craft(CraftStatus::Done)
                | JobStatus::Review(ReviewStatus::Done)
                | JobStatus::Orchestrator(MechanizedStatus::Done)
                | JobStatus::Operator(MechanizedStatus::Done)
        )
    }

    /// Returns true if the worker is actively working on this job.
    pub fn is_in_progress(&self) -> bool {
        matches!(
            self,
            JobStatus::Craft(CraftStatus::InProgress)
                | JobStatus::Review(ReviewStatus::InProgress)
                | JobStatus::Orchestrator(MechanizedStatus::InProgress)
                | JobStatus::Operator(MechanizedStatus::InProgress)
        )
    }
}

impl From<CraftStatus> for JobStatus {
    fn from(s: CraftStatus) -> Self {
        JobStatus::Craft(s)
    }
}

impl From<ReviewStatus> for JobStatus {
    fn from(s: ReviewStatus) -> Self {
        JobStatus::Review(s)
    }
}

impl fmt::Display for JobStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}
