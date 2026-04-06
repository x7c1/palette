mod mock_blueprint_reader;
mod mock_container_runtime;
mod mock_data_store;
mod mock_terminal_session;

pub use mock_blueprint_reader::MockBlueprintReader;
pub use mock_container_runtime::MockContainerRuntime;
pub use mock_data_store::MockDataStore;
pub use mock_terminal_session::MockTerminalSession;

use palette_domain::job::{Job, JobId, JobStatus, JobType};
use palette_domain::task::TaskId;
use palette_domain::terminal::TerminalTarget;
use palette_domain::worker::{ContainerId, WorkerId, WorkerRole, WorkerState, WorkerStatus};
use palette_domain::workflow::WorkflowId;

pub fn make_worker(id: &str, role: WorkerRole, status: WorkerStatus) -> WorkerState {
    WorkerState {
        id: WorkerId::parse(id).unwrap(),
        workflow_id: WorkflowId::parse("wf-test").unwrap(),
        role,
        supervisor_id: Some(WorkerId::parse("sup-1").unwrap()),
        container_id: ContainerId::new(format!("container-{id}")),
        terminal_target: TerminalTarget::new(format!("pane-{id}")),
        status,
        session_id: None,
        task_id: TaskId::parse(format!("wf-test:{id}")).unwrap(),
    }
}

pub fn make_job(id: &str) -> Job {
    use chrono::Utc;
    let now = Utc::now();
    Job {
        id: JobId::parse(id).unwrap(),
        task_id: TaskId::parse(format!("wf-test:{id}")).unwrap(),
        title: palette_domain::job::Title::parse(id).unwrap(),
        plan_path: palette_domain::job::PlanPath::parse(format!("test/{id}")).unwrap(),
        assignee_id: None,
        status: JobStatus::todo(JobType::Review),
        priority: None,
        detail: palette_domain::job::JobDetail::Review { perspective: None },
        created_at: now,
        updated_at: now,
        notes: None,
        assigned_at: None,
    }
}
