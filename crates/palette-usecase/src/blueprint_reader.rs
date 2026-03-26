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
    ) -> Result<TaskTree, Box<dyn std::error::Error + Send + Sync>>;
}
