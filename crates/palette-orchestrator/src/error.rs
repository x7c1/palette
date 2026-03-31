use palette_domain::task::TaskId;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    /// Error from an external dependency (data store, container runtime, etc.).
    External(Box<dyn std::error::Error + Send + Sync>),
    /// Referenced task does not exist.
    TaskNotFound { task_id: TaskId },
    /// Task is in an unexpected state for the requested operation.
    InvalidTaskState { task_id: TaskId, detail: String },
}

impl From<Box<dyn std::error::Error + Send + Sync>> for Error {
    fn from(e: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::External(e)
    }
}

impl From<palette_usecase::TaskStoreError> for Error {
    fn from(e: palette_usecase::TaskStoreError) -> Self {
        Self::External(Box::new(e))
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::External(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::External(e) => write!(f, "{e}"),
            Self::TaskNotFound { task_id } => write!(f, "task not found: {task_id}"),
            Self::InvalidTaskState { task_id, detail } => {
                write!(f, "invalid task state for {task_id}: {detail}")
            }
        }
    }
}
