use chrono::{DateTime, Utc};

use super::{WorkflowId, WorkflowStatus};

/// A Workflow is an execution of a Blueprint. It tracks the runtime state
/// of the Task tree defined in the Blueprint.
#[derive(Debug, Clone)]
pub struct Workflow {
    pub id: WorkflowId,
    pub blueprint_yaml: String,
    pub status: WorkflowStatus,
    pub started_at: DateTime<Utc>,
}
