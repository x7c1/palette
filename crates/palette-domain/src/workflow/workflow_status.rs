use std::str::FromStr;

/// Status of a Workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowStatus {
    /// Workflow is running.
    Active,
    /// Workflow is paused (e.g., due to an Escalation).
    Suspended,
    /// All Tasks in the Workflow are complete.
    Completed,
}

impl WorkflowStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkflowStatus::Active => "active",
            WorkflowStatus::Suspended => "suspended",
            WorkflowStatus::Completed => "completed",
        }
    }
}

impl FromStr for WorkflowStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(WorkflowStatus::Active),
            "suspended" => Ok(WorkflowStatus::Suspended),
            "completed" => Ok(WorkflowStatus::Completed),
            _ => Err(format!("invalid workflow status: {s}")),
        }
    }
}
