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
    /// Workflow stopped due to a runtime failure (e.g. workspace setup failed,
    /// branch conflict). The accompanying `failure_reason` carries the
    /// machine-readable reason key.
    Failed,
}

impl WorkflowStatus {
    pub fn parse(s: &str) -> Result<Self, InvalidWorkflowStatus> {
        match s {
            "active" => Ok(Self::Active),
            "suspending" => Ok(Self::Suspending),
            "suspended" => Ok(Self::Suspended),
            "completed" => Ok(Self::Completed),
            "terminated" => Ok(Self::Terminated),
            "failed" => Ok(Self::Failed),
            _ => Err(InvalidWorkflowStatus::Unknown {
                value: s.to_string(),
            }),
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Active,
            Self::Suspending,
            Self::Suspended,
            Self::Completed,
            Self::Terminated,
            Self::Failed,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            WorkflowStatus::Active => "active",
            WorkflowStatus::Suspending => "suspending",
            WorkflowStatus::Suspended => "suspended",
            WorkflowStatus::Completed => "completed",
            WorkflowStatus::Terminated => "terminated",
            WorkflowStatus::Failed => "failed",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_failed() {
        assert_eq!(
            WorkflowStatus::parse("failed").unwrap(),
            WorkflowStatus::Failed
        );
    }

    #[test]
    fn as_str_returns_failed_for_failed_variant() {
        assert_eq!(WorkflowStatus::Failed.as_str(), "failed");
    }

    #[test]
    fn all_contains_failed() {
        assert!(WorkflowStatus::all().contains(&WorkflowStatus::Failed));
    }
}
