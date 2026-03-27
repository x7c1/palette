mod mock_blueprint_reader;
mod mock_container_runtime;
mod mock_data_store;
mod mock_terminal_session;

pub use mock_blueprint_reader::MockBlueprintReader;
pub use mock_container_runtime::MockContainerRuntime;
pub use mock_data_store::MockDataStore;
pub use mock_terminal_session::MockTerminalSession;

use palette_domain::terminal::TerminalTarget;
use palette_domain::worker::{ContainerId, WorkerId, WorkerRole, WorkerState, WorkerStatus};
use palette_domain::workflow::WorkflowId;

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
