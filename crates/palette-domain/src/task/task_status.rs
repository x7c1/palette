use std::str::FromStr;

/// Status of a Task within a Workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is defined but not yet ready to start (dependencies not met).
    Pending,
    /// All dependencies are met; Task is ready to begin.
    Ready,
    /// Task is actively being worked on.
    InProgress,
    /// Task is paused due to an Escalation.
    Suspended,
    /// Task and all its child Tasks / Job are complete.
    Completed,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Ready => "ready",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Suspended => "suspended",
            TaskStatus::Completed => "completed",
        }
    }
}

impl FromStr for TaskStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(TaskStatus::Pending),
            "ready" => Ok(TaskStatus::Ready),
            "in_progress" => Ok(TaskStatus::InProgress),
            "suspended" => Ok(TaskStatus::Suspended),
            "completed" => Ok(TaskStatus::Completed),
            _ => Err(format!("invalid task status: {s}")),
        }
    }
}
