use chrono::{DateTime, Utc};

use super::{WorkflowId, WorkflowStatus};

/// A Workflow is an execution of a Blueprint. It tracks the runtime state
/// of the Task tree defined in the Blueprint.
#[derive(Debug, Clone)]
pub struct Workflow {
    pub id: WorkflowId,
    pub blueprint_path: String,
    pub status: WorkflowStatus,
    pub started_at: DateTime<Utc>,
    /// SHA-256 hash of the Blueprint file at the time of the last apply.
    /// None if no apply has been performed (Blueprint unchanged since start).
    pub blueprint_hash: Option<String>,
    /// Machine-readable reason key (`{namespace}/{value}`) set when `status`
    /// transitions to [`WorkflowStatus::Failed`]. `None` for all other states.
    pub failure_reason: Option<String>,
}
