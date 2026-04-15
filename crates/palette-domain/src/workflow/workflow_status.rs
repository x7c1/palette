use std::fmt;

/// Status of a Workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowStatus {
    /// Workflow is running.
    Active,
    /// Suspend requested; waiting for in-progress workers to finish.
    /// New job assignments and message delivery are blocked.
    Suspending,
    /// Workflow is paused (all workers stopped).
    Suspended,
    /// All Tasks in the Workflow are complete.
    Completed,
    /// Workflow was terminated by an explicit Orchestrator shutdown.
    /// Workers have been destroyed and cannot be resumed.
    Terminated,
}

impl WorkflowStatus {
    pub fn parse(s: &str) -> Result<Self, InvalidWorkflowStatus> {
        match s {
            "active" => Ok(Self::Active),
            "suspending" => Ok(Self::Suspending),
            "suspended" => Ok(Self::Suspended),
            "completed" => Ok(Self::Completed),
            "terminated" => Ok(Self::Terminated),
            _ => Err(InvalidWorkflowStatus::Unknown {
                value: s.to_string(),
            }),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            WorkflowStatus::Active => "active",
            WorkflowStatus::Suspending => "suspending",
            WorkflowStatus::Suspended => "suspended",
            WorkflowStatus::Completed => "completed",
            WorkflowStatus::Terminated => "terminated",
        }
    }
}

impl fmt::Display for WorkflowStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

#[derive(Debug, palette_macros::ReasonKey)]
pub enum InvalidWorkflowStatus {
    Unknown { value: String },
}
