use super::JobType;
use super::craft_status::CraftStatus;
use super::review_status::ReviewStatus;

/// Typed job status that pairs with the job type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Craft(CraftStatus),
    Review(ReviewStatus),
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Craft(s) => s.as_str(),
            JobStatus::Review(s) => s.as_str(),
        }
    }

    /// Parse a status string using the job type to determine the variant.
    pub fn parse(s: &str, job_type: JobType) -> Result<Self, String> {
        match job_type {
            JobType::Craft => s.parse::<CraftStatus>().map(JobStatus::Craft),
            JobType::Review => s.parse::<ReviewStatus>().map(JobStatus::Review),
        }
    }

    /// Create a Todo status for the given job type.
    pub fn todo(job_type: JobType) -> Self {
        match job_type {
            JobType::Craft => JobStatus::Craft(CraftStatus::Todo),
            JobType::Review => JobStatus::Review(ReviewStatus::Todo),
        }
    }

    /// Create an InProgress status for the given job type.
    pub fn in_progress(job_type: JobType) -> Self {
        match job_type {
            JobType::Craft => JobStatus::Craft(CraftStatus::InProgress),
            JobType::Review => JobStatus::Review(ReviewStatus::InProgress),
        }
    }

    /// Returns true if the job is done (Craft::Done or Review::Done).
    pub fn is_done(&self) -> bool {
        matches!(
            self,
            JobStatus::Craft(CraftStatus::Done) | JobStatus::Review(ReviewStatus::Done)
        )
    }
}
