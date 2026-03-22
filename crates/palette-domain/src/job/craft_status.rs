use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftStatus {
    Todo,
    InProgress,
    InReview,
    Done,
    Escalated,
}

impl CraftStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CraftStatus::Todo => "todo",
            CraftStatus::InProgress => "in_progress",
            CraftStatus::InReview => "in_review",
            CraftStatus::Done => "done",
            CraftStatus::Escalated => "escalated",
        }
    }
}

impl FromStr for CraftStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "todo" => Ok(CraftStatus::Todo),
            "in_progress" => Ok(CraftStatus::InProgress),
            "in_review" => Ok(CraftStatus::InReview),
            "done" => Ok(CraftStatus::Done),
            "escalated" => Ok(CraftStatus::Escalated),
            _ => Err(format!("invalid craft status: {s}")),
        }
    }
}
