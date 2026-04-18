use super::ids::wid;
use palette_domain::task::TaskId;
use palette_domain::terminal::TerminalTarget;
use palette_domain::worker::{ContainerId, WorkerId, WorkerRole, WorkerStatus};
use palette_domain::workflow::WorkflowId;
use palette_usecase::InsertWorkerRequest;

/// Insert a worker record to satisfy FK constraints.
pub fn setup_worker(db: &dyn palette_usecase::DataStore, worker_id: &str) {
    let wf_id = WorkflowId::parse("wf-test").unwrap();
    let _ = db.create_workflow(&wf_id, "test/blueprint.yaml");
    db.insert_worker(&InsertWorkerRequest {
        id: WorkerId::parse(worker_id).unwrap(),
        workflow_id: wf_id,
        role: WorkerRole::Member,
        status: WorkerStatus::Booting,
        supervisor_id: None,
        container_id: ContainerId::new(format!("container-{worker_id}")),
        terminal_target: TerminalTarget::new(format!("pane-{worker_id}")),
        session_id: None,
        task_id: TaskId::parse(format!("wf-test:{worker_id}")).unwrap(),
    })
    .unwrap();
}

/// Insert a worker with full control over all fields.
#[allow(clippy::too_many_arguments)]
pub fn insert_worker(
    state: &palette_server::AppState,
    id: &str,
    role: WorkerRole,
    supervisor_id: Option<&str>,
    terminal_target: &TerminalTarget,
    status: WorkerStatus,
    task_id: &str,
    workflow_id: &WorkflowId,
) {
    state
        .interactor
        .data_store
        .insert_worker(&InsertWorkerRequest {
            id: wid(id),
            workflow_id: workflow_id.clone(),
            role,
            status,
            supervisor_id: supervisor_id.map(wid),
            container_id: ContainerId::new("stub"),
            terminal_target: terminal_target.clone(),
            session_id: None,
            task_id: TaskId::parse(task_id).unwrap(),
        })
        .unwrap();
}
