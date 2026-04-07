use crate::blueprint_reader::ReadBlueprintError;
use palette_domain::workflow::WorkflowId;
use std::fmt;

#[derive(Debug)]
pub enum TaskStoreError {
    /// Data store operation failed.
    DataStore(Box<dyn std::error::Error + Send + Sync>),
    /// Blueprint read or validation failed.
    Blueprint(ReadBlueprintError),
    /// Referenced workflow does not exist.
    WorkflowNotFound { workflow_id: WorkflowId },
    /// Workflow has no blueprint_path (e.g., PR review workflow).
    BlueprintNotAvailable { workflow_id: WorkflowId },
}

impl fmt::Display for TaskStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskStoreError::DataStore(e) => write!(f, "data store error: {e}"),
            TaskStoreError::Blueprint(e) => write!(f, "blueprint error: {e}"),
            TaskStoreError::WorkflowNotFound { workflow_id } => {
                write!(f, "workflow not found: {workflow_id}")
            }
            TaskStoreError::BlueprintNotAvailable { workflow_id } => {
                write!(f, "blueprint not available for workflow: {workflow_id}")
            }
        }
    }
}

impl std::error::Error for TaskStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TaskStoreError::DataStore(e) => Some(e.as_ref()),
            TaskStoreError::Blueprint(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ReadBlueprintError> for TaskStoreError {
    fn from(e: ReadBlueprintError) -> Self {
        TaskStoreError::Blueprint(e)
    }
}
