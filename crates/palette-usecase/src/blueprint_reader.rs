use palette_core::InputError;
use palette_domain::task::TaskTree;
use palette_domain::workflow::WorkflowId;
use std::path::{Path, PathBuf};

/// Port for reading and validating blueprint files.
///
/// Abstracts filesystem access for blueprint YAML parsing so that
/// the orchestrator and server can be tested with mock implementations.
pub trait BlueprintReader: Send + Sync {
    fn read_blueprint(
        &self,
        path: &Path,
        workflow_id: &WorkflowId,
    ) -> Result<TaskTree, ReadBlueprintError>;
}

/// Error returned by [`BlueprintReader::read_blueprint`].
#[derive(Debug)]
pub enum ReadBlueprintError {
    /// Blueprint file does not exist at the given path.
    NotFound { path: PathBuf },
    /// Blueprint content failed validation (YAML parse, structural rules,
    /// missing referenced files). Contains machine-readable `InputError`s.
    Invalid(Vec<InputError>),
    /// Unexpected I/O or adapter-internal error. Callers should surface this
    /// as a 500-class failure.
    Internal(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for ReadBlueprintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadBlueprintError::NotFound { path } => {
                write!(f, "blueprint not found: {}", path.display())
            }
            ReadBlueprintError::Invalid(errors) => {
                let reasons: Vec<&str> = errors.iter().map(|e| e.reason.as_str()).collect();
                write!(f, "blueprint validation failed: {}", reasons.join(", "))
            }
            ReadBlueprintError::Internal(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ReadBlueprintError {}
