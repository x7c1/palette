use palette_domain::task::TaskTree;
use palette_domain::workflow::WorkflowId;
use palette_usecase::{BlueprintReader, ReadBlueprintError};
use std::path::Path;

pub(in crate::admin) struct NoopBlueprint;

impl BlueprintReader for NoopBlueprint {
    fn read_blueprint(
        &self,
        _path: &Path,
        _workflow_id: &WorkflowId,
    ) -> Result<TaskTree, ReadBlueprintError> {
        Err(ReadBlueprintError::Read(
            std::io::Error::other("read_blueprint is not available in admin mode").into(),
        ))
    }
}
