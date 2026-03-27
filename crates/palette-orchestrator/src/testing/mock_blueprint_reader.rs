use palette_domain::task::TaskTree;
use palette_domain::workflow::WorkflowId;
use palette_usecase::BlueprintReader;

pub struct MockBlueprintReader;

impl BlueprintReader for MockBlueprintReader {
    fn read_blueprint(
        &self,
        _path: &std::path::Path,
        _workflow_id: &WorkflowId,
    ) -> Result<TaskTree, Box<dyn std::error::Error + Send + Sync>> {
        unimplemented!()
    }
}
