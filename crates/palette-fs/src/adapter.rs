use palette_core::{InputError, Location, ReasonKey};
use palette_domain::task::TaskTree;
use palette_domain::workflow::WorkflowId;
use palette_usecase::BlueprintReader;
use palette_usecase::blueprint_reader::ReadBlueprintError;
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
    ) -> Result<TaskTree, ReadBlueprintError> {
        let blueprint =
            crate::read_blueprint(path).map_err(|e| ReadBlueprintError::Read(Box::new(e)))?;
        let tree = blueprint.to_task_tree(workflow_id).map_err(|errors| {
            ReadBlueprintError::Validation(
                errors
                    .iter()
                    .map(|e| InputError {
                        location: Location::Body,
                        hint: e.field_path(),
                        reason: e.reason_key(),
                    })
                    .collect(),
            )
        })?;
        Ok(tree)
    }
}
