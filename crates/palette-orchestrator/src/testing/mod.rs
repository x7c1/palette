mod mock_container_runtime;
mod mock_data_store;
mod mock_terminal_session;

pub use mock_container_runtime::MockContainerRuntime;
pub use mock_data_store::MockDataStore;
pub use mock_terminal_session::MockTerminalSession;

use palette_domain::task::TaskTree;
use palette_domain::terminal::TerminalTarget;
use palette_domain::worker::{ContainerId, WorkerId, WorkerRole, WorkerState, WorkerStatus};
use palette_domain::workflow::WorkflowId;
use palette_usecase::BlueprintReader;

pub fn make_worker(id: &str, role: WorkerRole, status: WorkerStatus) -> WorkerState {
    WorkerState {
        id: WorkerId::new(id),
        workflow_id: WorkflowId::new("wf-test"),
        role,
        supervisor_id: WorkerId::new("sup-1"),
        container_id: ContainerId::new(format!("container-{id}")),
        terminal_target: TerminalTarget::new(format!("pane-{id}")),
        status,
        session_id: None,
        task_id: palette_domain::task::TaskId::new(format!("task-{id}")),
    }
}

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
