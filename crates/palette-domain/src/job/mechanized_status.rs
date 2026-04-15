use std::fmt;

/// Status for mechanized jobs (Orchestrator and Operator).
///
/// Unlike Craft/Review, these jobs do not spawn worker containers.
/// Orchestrator jobs run commands on the host; Operator jobs wait for human input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MechanizedStatus {
    Todo,
    InProgress,
    Done,
    Failed,
    Terminated,
}

impl MechanizedStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            MechanizedStatus::Todo => "todo",
            MechanizedStatus::InProgress => "in_progress",
            MechanizedStatus::Done => "done",
            MechanizedStatus::Failed => "failed",
            MechanizedStatus::Terminated => "terminated",
        }
    }
}

impl fmt::Display for MechanizedStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}
