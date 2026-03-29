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

pub fn jid(s: &str) -> JobId {
    JobId::new(s)
}

pub fn tid(s: &str) -> TaskId {
    TaskId::new(s)
}

pub fn wid(s: &str) -> WorkerId {
    WorkerId::new(s)
}

/// Create a workflow and a task for testing. Returns the TaskId.
pub fn setup_task(db: &Database, task_id: &str) -> TaskId {
    let wf_id = WorkflowId::new(format!("wf-{task_id}"));
    let t_id = tid(task_id);
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
    let wf_id = WorkflowId::new("wf-test");
    let _ = db.create_workflow(&wf_id, "test/blueprint.yaml");
    db.insert_worker(&InsertWorkerRequest {
        id: WorkerId::new(worker_id),
        workflow_id: wf_id,
        role: WorkerRole::Member,
        status: WorkerStatus::Booting,
        supervisor_id: None,
        container_id: ContainerId::new(format!("container-{worker_id}")),
        terminal_target: TerminalTarget::new(format!("pane-{worker_id}")),
        session_id: None,
        task_id: TaskId::new(format!("task-{worker_id}")),
    })
    .unwrap();
}

pub fn create_craft(db: &Database, id: &str, priority: Option<Priority>) {
    let task_id = setup_task(db, &format!("task-{id}"));
    db.create_job(&CreateJobRequest {
        task_id,
        id: Some(jid(id)),
        job_type: JobType::Craft,
        title: format!("Job {id}"),
        plan_path: format!("test/{id}"),
        assignee_id: None,
        priority,
        repository: None,
    })
    .unwrap();
}

pub fn create_review(db: &Database, id: &str) {
    let task_id = setup_task(db, &format!("task-{id}"));
    db.create_job(&CreateJobRequest {
        task_id,
        id: Some(jid(id)),
        job_type: JobType::Review,
        title: format!("Review {id}"),
        plan_path: format!("test/{id}"),
        assignee_id: None,
        priority: None,
        repository: None,
    })
    .unwrap();
}
