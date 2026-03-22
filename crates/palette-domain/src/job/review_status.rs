use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewStatus {
    Todo,
    InProgress,
    ChangesRequested,
    Done,
    Escalated,
}

impl ReviewStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReviewStatus::Todo => "todo",
            ReviewStatus::InProgress => "in_progress",
            ReviewStatus::ChangesRequested => "changes_requested",
            ReviewStatus::Done => "done",
            ReviewStatus::Escalated => "escalated",
        }
    }
}

impl FromStr for ReviewStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "todo" => Ok(ReviewStatus::Todo),
            "in_progress" => Ok(ReviewStatus::InProgress),
            "changes_requested" => Ok(ReviewStatus::ChangesRequested),
            "done" => Ok(ReviewStatus::Done),
            "escalated" => Ok(ReviewStatus::Escalated),
            _ => Err(format!("invalid review status: {s}")),
        }
    }
}
