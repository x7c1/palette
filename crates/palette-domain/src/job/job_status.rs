use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Draft => "draft",
            JobStatus::Ready => "ready",
            JobStatus::Todo => "todo",
            JobStatus::InProgress => "in_progress",
            JobStatus::InReview => "in_review",
            JobStatus::Done => "done",
            JobStatus::Blocked => "blocked",
            JobStatus::Escalated => "escalated",
        }
    }
}

impl FromStr for JobStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "draft" => Ok(JobStatus::Draft),
            "ready" => Ok(JobStatus::Ready),
            "todo" => Ok(JobStatus::Todo),
            "in_progress" => Ok(JobStatus::InProgress),
            "in_review" => Ok(JobStatus::InReview),
            "done" => Ok(JobStatus::Done),
            "blocked" => Ok(JobStatus::Blocked),
            "escalated" => Ok(JobStatus::Escalated),
            _ => Err(format!("invalid job status: {s}")),
        }
    }
}
