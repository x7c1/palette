use palette_domain as domain;
use serde::{Deserialize, Serialize};

/// API-level job status. Flat enum for JSON serialization.
/// Conversion to/from domain types requires the job type context.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Todo,
    InProgress,
    InReview,
    ChangesRequested,
    Done,
    Escalated,
    Failed,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Todo => "todo",
            JobStatus::InProgress => "in_progress",
            JobStatus::InReview => "in_review",
            JobStatus::ChangesRequested => "changes_requested",
            JobStatus::Done => "done",
            JobStatus::Escalated => "escalated",
            JobStatus::Failed => "failed",
        }
    }

    /// Convert API status to domain status using the job type for disambiguation.
    pub fn to_domain(self, job_type: domain::job::JobType) -> domain::job::JobStatus {
        match job_type {
            domain::job::JobType::Craft => {
                let craft = match self {
                    JobStatus::Todo => domain::job::CraftStatus::Todo,
                    JobStatus::InProgress => domain::job::CraftStatus::InProgress,
                    JobStatus::InReview => domain::job::CraftStatus::InReview,
                    JobStatus::Done => domain::job::CraftStatus::Done,
                    JobStatus::Escalated => domain::job::CraftStatus::Escalated,
                    // ChangesRequested/Failed are not valid for craft, but map to Escalated as fallback
                    JobStatus::ChangesRequested | JobStatus::Failed => {
                        domain::job::CraftStatus::Escalated
                    }
                };
                domain::job::JobStatus::Craft(craft)
            }
            domain::job::JobType::Review | domain::job::JobType::ReviewIntegrate => {
                let review = match self {
                    JobStatus::Todo => domain::job::ReviewStatus::Todo,
                    JobStatus::InProgress => domain::job::ReviewStatus::InProgress,
                    JobStatus::ChangesRequested => domain::job::ReviewStatus::ChangesRequested,
                    JobStatus::Done => domain::job::ReviewStatus::Done,
                    JobStatus::Escalated => domain::job::ReviewStatus::Escalated,
                    // InReview/Failed are not valid for review, but map to InProgress as fallback
                    JobStatus::InReview | JobStatus::Failed => {
                        domain::job::ReviewStatus::InProgress
                    }
                };
                domain::job::JobStatus::Review(review)
            }
            domain::job::JobType::Orchestrator | domain::job::JobType::Operator => {
                let ms = match self {
                    JobStatus::Todo => domain::job::MechanizedStatus::Todo,
                    JobStatus::InProgress | JobStatus::InReview | JobStatus::ChangesRequested => {
                        domain::job::MechanizedStatus::InProgress
                    }
                    JobStatus::Done | JobStatus::Escalated => domain::job::MechanizedStatus::Done,
                    JobStatus::Failed => domain::job::MechanizedStatus::Failed,
                };
                if job_type == domain::job::JobType::Orchestrator {
                    domain::job::JobStatus::Orchestrator(ms)
                } else {
                    domain::job::JobStatus::Operator(ms)
                }
            }
        }
    }
}

impl From<domain::job::JobStatus> for JobStatus {
    fn from(s: domain::job::JobStatus) -> Self {
        match s {
            domain::job::JobStatus::Craft(cs) => match cs {
                domain::job::CraftStatus::Todo => JobStatus::Todo,
                domain::job::CraftStatus::InProgress => JobStatus::InProgress,
                domain::job::CraftStatus::InReview => JobStatus::InReview,
                domain::job::CraftStatus::Done => JobStatus::Done,
                domain::job::CraftStatus::Escalated => JobStatus::Escalated,
            },
            domain::job::JobStatus::Review(rs) => match rs {
                domain::job::ReviewStatus::Todo => JobStatus::Todo,
                domain::job::ReviewStatus::InProgress => JobStatus::InProgress,
                domain::job::ReviewStatus::ChangesRequested => JobStatus::ChangesRequested,
                domain::job::ReviewStatus::Done => JobStatus::Done,
                domain::job::ReviewStatus::Escalated => JobStatus::Escalated,
            },
            domain::job::JobStatus::Orchestrator(ms) | domain::job::JobStatus::Operator(ms) => {
                match ms {
                    domain::job::MechanizedStatus::Todo => JobStatus::Todo,
                    domain::job::MechanizedStatus::InProgress => JobStatus::InProgress,
                    domain::job::MechanizedStatus::Done => JobStatus::Done,
                    domain::job::MechanizedStatus::Failed => JobStatus::Failed,
                }
            }
        }
    }
}
