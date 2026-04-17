use crate::blueprint_reader::ReadBlueprintError;
use std::fmt;

#[derive(Debug)]
pub enum TaskStoreError {
    /// Data store operation failed.
    DataStore(Box<dyn std::error::Error + Send + Sync>),
    /// Blueprint read or validation failed.
    Blueprint(ReadBlueprintError),
}

impl fmt::Display for TaskStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskStoreError::DataStore(e) => write!(f, "data store error: {e}"),
            TaskStoreError::Blueprint(e) => write!(f, "blueprint error: {e}"),
        }
    }
}

impl std::error::Error for TaskStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TaskStoreError::DataStore(e) => Some(e.as_ref()),
            TaskStoreError::Blueprint(e) => Some(e),
        }
    }
}

impl From<ReadBlueprintError> for TaskStoreError {
    fn from(e: ReadBlueprintError) -> Self {
        TaskStoreError::Blueprint(e)
    }
}
