use palette_core::FieldError;
use palette_domain::task::TaskTree;
use palette_domain::workflow::WorkflowId;
use std::path::Path;

/// Port for reading blueprint files.
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
    /// Blueprint file could not be read or parsed.
    Read(Box<dyn std::error::Error + Send + Sync>),
    /// Blueprint content violates structural constraints.
    Validation(Vec<FieldError>),
}

impl std::fmt::Display for ReadBlueprintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadBlueprintError::Read(e) => write!(f, "{e}"),
            ReadBlueprintError::Validation(errors) => {
                let reasons: Vec<&str> = errors.iter().map(|e| e.reason.as_str()).collect();
                write!(f, "blueprint validation failed: {}", reasons.join(", "))
            }
        }
    }
}

impl std::error::Error for ReadBlueprintError {}
