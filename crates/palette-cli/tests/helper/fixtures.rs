use palette_domain::job::JobId;
use palette_domain::worker::WorkerId;
use palette_server::api_types::{CreateJobRequest, JobStatus, JobType, UpdateJobRequest};

pub fn wid(s: &str) -> WorkerId {
    WorkerId::parse(s).unwrap()
}

pub fn jid(s: &str) -> JobId {
    JobId::parse(s).unwrap()
}

/// Insert a worker record to satisfy FK constraints.
pub fn setup_worker(db: &dyn palette_usecase::DataStore, worker_id: &str) {
    use palette_domain::task::TaskId;
    use palette_domain::terminal::TerminalTarget;
    use palette_domain::worker::*;
    use palette_domain::workflow::WorkflowId;
    use palette_usecase::data_store::InsertWorkerRequest;

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

pub fn create_craft(id: &str, title: &str, task_id: &str) -> CreateJobRequest {
    CreateJobRequest {
        id: Some(id.to_string()),
        task_id: task_id.to_string(),
        job_type: JobType::Craft,
        title: title.to_string(),
        plan_path: format!("test/{id}"),
        assignee_id: None,
        priority: None,
        repository: None,
        command: None,
    }
}

pub fn create_review(id: &str, title: &str, task_id: &str) -> CreateJobRequest {
    CreateJobRequest {
        id: Some(id.to_string()),
        task_id: task_id.to_string(),
        job_type: JobType::Review,
        title: title.to_string(),
        plan_path: format!("test/{id}"),
        assignee_id: None,
        priority: None,
        repository: None,
        command: None,
    }
}

pub fn update_status(id: &str, status: JobStatus) -> UpdateJobRequest {
    UpdateJobRequest {
        id: id.to_string(),
        status,
    }
}
