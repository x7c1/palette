use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewStatus {
    Todo,
    InProgress,
    ChangesRequested,
    Done,
    Escalated,
    Terminated,
}

impl ReviewStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReviewStatus::Todo => "todo",
            ReviewStatus::InProgress => "in_progress",
            ReviewStatus::ChangesRequested => "changes_requested",
            ReviewStatus::Done => "done",
            ReviewStatus::Escalated => "escalated",
            ReviewStatus::Terminated => "terminated",
        }
    }
}

impl fmt::Display for ReviewStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}
