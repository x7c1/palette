use super::Database;
use crate::InsertWorkerRequest;
use palette_domain::job::*;
use palette_domain::task::TaskId;
use palette_domain::terminal::TerminalTarget;
use palette_domain::worker::*;
use palette_domain::workflow::WorkflowId;

use super::task::CreateTaskRequest;

pub fn test_db() -> Database {
    Database::open_in_memory().unwrap()
}

pub fn wid(s: &str) -> WorkerId {
    WorkerId::parse(s).unwrap()
}

/// Create a workflow and a task for testing. Returns the TaskId.
///
/// `task_id` must contain `:` (e.g. `"wf-test:task-1"`).
/// The workflow ID is extracted from the portion before the first `:`.
pub fn setup_task(db: &Database, task_id: &str) -> TaskId {
    let t_id = TaskId::parse(task_id).unwrap();
    let wf_part = task_id.split(':').next().unwrap();
    let wf_id = WorkflowId::parse(wf_part).unwrap();
    // Ignore errors if workflow already exists
    let _ = db.create_workflow(&wf_id, "test/blueprint.yaml");
    let _ = db.create_task(&CreateTaskRequest {
        id: t_id.clone(),
        workflow_id: wf_id,
    });
    t_id
}

/// Insert a worker record for FK-constrained tests.
pub fn setup_worker(db: &Database, worker_id: &str) {
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

pub fn create_craft(db: &Database, id: &str, priority: Option<Priority>) -> Job {
    let task_id = setup_task(db, &format!("wf-test:task-{id}"));
    db.create_job(&CreateJobRequest::new(
        task_id,
        Title::parse(format!("Job {id}")).unwrap(),
        Some(PlanPath::parse(format!("test/{id}")).unwrap()),
        None,
        priority,
        JobDetail::Craft {
            repository: Repository::parse("x7c1/palette-demo", "main", None).unwrap(),
        },
    ))
    .unwrap()
}

pub fn create_review(db: &Database, id: &str) -> Job {
    let task_id = setup_task(db, &format!("wf-test:task-{id}"));
    db.create_job(&CreateJobRequest::new(
        task_id,
        Title::parse(format!("Review {id}")).unwrap(),
        Some(PlanPath::parse(format!("test/{id}")).unwrap()),
        None,
        None,
        JobDetail::Review {
            perspective: None,
            target: ReviewTarget::CraftOutput,
        },
    ))
    .unwrap()
}
