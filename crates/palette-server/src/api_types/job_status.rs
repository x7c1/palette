use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Draft,
    Ready,
    Todo,
    InProgress,
    InReview,
    Done,
    Blocked,
    Escalated,
}

impl From<JobStatus> for domain::job::JobStatus {
    fn from(s: JobStatus) -> Self {
        match s {
            JobStatus::Draft => domain::job::JobStatus::Draft,
            JobStatus::Ready => domain::job::JobStatus::Ready,
            JobStatus::Todo => domain::job::JobStatus::Todo,
            JobStatus::InProgress => domain::job::JobStatus::InProgress,
            JobStatus::InReview => domain::job::JobStatus::InReview,
            JobStatus::Done => domain::job::JobStatus::Done,
            JobStatus::Blocked => domain::job::JobStatus::Blocked,
            JobStatus::Escalated => domain::job::JobStatus::Escalated,
        }
    }
}

impl From<domain::job::JobStatus> for JobStatus {
    fn from(s: domain::job::JobStatus) -> Self {
        match s {
            domain::job::JobStatus::Draft => JobStatus::Draft,
            domain::job::JobStatus::Ready => JobStatus::Ready,
            domain::job::JobStatus::Todo => JobStatus::Todo,
            domain::job::JobStatus::InProgress => JobStatus::InProgress,
            domain::job::JobStatus::InReview => JobStatus::InReview,
            domain::job::JobStatus::Done => JobStatus::Done,
            domain::job::JobStatus::Blocked => JobStatus::Blocked,
            domain::job::JobStatus::Escalated => JobStatus::Escalated,
        }
    }
}
