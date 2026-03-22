use super::craft_status::CraftStatus;
use super::review_status::ReviewStatus;
use super::JobType;

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

    /// Returns true if the job is done (Craft::Done or Review::Done).
    pub fn is_done(&self) -> bool {
        matches!(
            self,
            JobStatus::Craft(CraftStatus::Done) | JobStatus::Review(ReviewStatus::Done)
        )
    }
}
