use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftStatus {
    Todo,
    InProgress,
    InReview,
    Done,
    Escalated,
    Terminated,
}

impl CraftStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CraftStatus::Todo => "todo",
            CraftStatus::InProgress => "in_progress",
            CraftStatus::InReview => "in_review",
            CraftStatus::Done => "done",
            CraftStatus::Escalated => "escalated",
            CraftStatus::Terminated => "terminated",
        }
    }
}

impl fmt::Display for CraftStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}
