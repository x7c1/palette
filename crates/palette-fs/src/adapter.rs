use palette_domain::task::TaskTree;
use palette_domain::workflow::WorkflowId;
use palette_usecase::BlueprintReader;
use std::path::Path;

/// Filesystem-backed blueprint reader.
///
/// Reads YAML blueprint files from the local filesystem.
pub struct FsBlueprintReader;

impl BlueprintReader for FsBlueprintReader {
    fn read_blueprint(
        &self,
        path: &Path,
        workflow_id: &WorkflowId,
    ) -> Result<TaskTree, Box<dyn std::error::Error + Send + Sync>> {
        let blueprint = crate::read_blueprint(path)?;
        let tree = blueprint.to_task_tree(workflow_id).map_err(|errors| {
            let messages: Vec<String> = errors
                .iter()
                .map(|e| format!("{}:{}", e.field_path(), e.reason_key()))
                .collect();
            messages.join("; ")
        })?;
        Ok(tree)
    }
}
