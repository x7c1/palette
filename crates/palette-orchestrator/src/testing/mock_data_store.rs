use palette_domain::job::{CreateJobRequest, Job, JobFilter, JobId, JobStatus, JobType};
use palette_domain::review::{ReviewComment, ReviewSubmission, SubmitReviewRequest};
use palette_domain::task::{TaskId, TaskState, TaskStatus};
use palette_domain::worker::{ContainerId, WorkerId, WorkerSessionId, WorkerState, WorkerStatus};
use palette_domain::workflow::{Workflow, WorkflowId, WorkflowStatus};
use palette_usecase::{CreateTaskRequest, DataStore, InsertWorkerRequest};
use std::collections::HashMap;
use std::sync::Mutex;

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

pub struct MockDataStore {
    pub workers: Mutex<Vec<WorkerState>>,
    pub messages: Mutex<HashMap<WorkerId, Vec<String>>>,
    pub status_updates: Mutex<Vec<(WorkerId, WorkerStatus)>>,
    pub assignable_jobs: Mutex<Vec<Job>>,
    pub jobs: Mutex<Vec<Job>>,
}

impl MockDataStore {
    pub fn new() -> Self {
        Self {
            workers: Mutex::new(Vec::new()),
            messages: Mutex::new(HashMap::new()),
            status_updates: Mutex::new(Vec::new()),
            assignable_jobs: Mutex::new(Vec::new()),
            jobs: Mutex::new(Vec::new()),
        }
    }

    pub fn with_workers(workers: Vec<WorkerState>) -> Self {
        let store = Self::new();
        *store.workers.lock().unwrap() = workers;
        store
    }
}

impl DataStore for MockDataStore {
    fn list_all_workers(&self) -> Result<Vec<WorkerState>, BoxErr> {
        Ok(self.workers.lock().unwrap().clone())
    }

    fn update_worker_status(&self, id: &WorkerId, status: WorkerStatus) -> Result<(), BoxErr> {
        self.status_updates
            .lock()
            .unwrap()
            .push((id.clone(), status));
        let mut workers = self.workers.lock().unwrap();
        if let Some(w) = workers.iter_mut().find(|w| w.id == *id) {
            w.status = status;
        }
        Ok(())
    }

    fn enqueue_message(&self, target_id: &WorkerId, message: &str) -> Result<(), BoxErr> {
        self.messages
            .lock()
            .unwrap()
            .entry(target_id.clone())
            .or_default()
            .push(message.to_string());
        Ok(())
    }

    fn has_pending_messages(&self, target_id: &WorkerId) -> Result<bool, BoxErr> {
        Ok(self
            .messages
            .lock()
            .unwrap()
            .get(target_id)
            .is_some_and(|m| !m.is_empty()))
    }

    fn find_worker(&self, id: &WorkerId) -> Result<Option<WorkerState>, BoxErr> {
        Ok(self
            .workers
            .lock()
            .unwrap()
            .iter()
            .find(|w| w.id == *id)
            .cloned())
    }

    fn insert_worker(&self, _: &InsertWorkerRequest) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn find_worker_by_container(&self, _: &ContainerId) -> Result<Option<WorkerState>, BoxErr> {
        unimplemented!()
    }
    fn list_supervisors(&self, _: &WorkflowId) -> Result<Vec<WorkerState>, BoxErr> {
        unimplemented!()
    }
    fn list_members(&self, _: &WorkflowId) -> Result<Vec<WorkerState>, BoxErr> {
        unimplemented!()
    }
    fn list_booting_workers(&self) -> Result<Vec<WorkerState>, BoxErr> {
        unimplemented!()
    }
    fn list_idle_or_waiting_workers(&self) -> Result<Vec<WorkerState>, BoxErr> {
        Ok(self
            .workers
            .lock()
            .unwrap()
            .iter()
            .filter(|w| {
                matches!(
                    w.status,
                    WorkerStatus::Idle | WorkerStatus::WaitingPermission
                )
            })
            .cloned()
            .collect())
    }
    fn update_worker_session_id(&self, _: &WorkerId, _: &WorkerSessionId) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn remove_worker(&self, _: &WorkerId) -> Result<Option<WorkerState>, BoxErr> {
        unimplemented!()
    }
    fn find_supervisor_for_task(&self, _: &TaskId) -> Result<Option<WorkerState>, BoxErr> {
        unimplemented!()
    }
    fn find_supervisors_for_task(&self, _: &TaskId) -> Result<Vec<WorkerState>, BoxErr> {
        unimplemented!()
    }
    fn create_job(&self, _: &CreateJobRequest) -> Result<Job, BoxErr> {
        unimplemented!()
    }
    fn get_job(&self, _: &JobId) -> Result<Option<Job>, BoxErr> {
        unimplemented!()
    }
    fn get_job_by_task_id(&self, _: &TaskId) -> Result<Option<Job>, BoxErr> {
        unimplemented!()
    }
    fn list_jobs(&self, _: &JobFilter) -> Result<Vec<Job>, BoxErr> {
        Ok(self.jobs.lock().unwrap().clone())
    }
    fn assign_job(&self, _: &JobId, _: &WorkerId, _: JobType) -> Result<Job, BoxErr> {
        unimplemented!()
    }
    fn update_job_status(&self, _: &JobId, _: JobStatus) -> Result<Job, BoxErr> {
        unimplemented!()
    }
    fn find_assignable_jobs(&self) -> Result<Vec<Job>, BoxErr> {
        Ok(self.assignable_jobs.lock().unwrap().clone())
    }
    fn count_active_workers(&self) -> Result<usize, BoxErr> {
        Ok(self.workers.lock().unwrap().len())
    }
    fn submit_review(
        &self,
        _: &JobId,
        _: &SubmitReviewRequest,
    ) -> Result<ReviewSubmission, BoxErr> {
        unimplemented!()
    }
    fn get_review_submissions(&self, _: &JobId) -> Result<Vec<ReviewSubmission>, BoxErr> {
        unimplemented!()
    }
    fn get_review_comments(&self, _: i64) -> Result<Vec<ReviewComment>, BoxErr> {
        unimplemented!()
    }
    fn create_task(&self, _: &CreateTaskRequest) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn get_task_state(&self, _: &TaskId) -> Result<Option<TaskState>, BoxErr> {
        unimplemented!()
    }
    fn get_task_statuses(&self, _: &WorkflowId) -> Result<HashMap<TaskId, TaskStatus>, BoxErr> {
        unimplemented!()
    }
    fn update_task_status(&self, _: &TaskId, _: TaskStatus) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn delete_task(&self, _: &TaskId) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn delete_jobs_by_task_id(&self, _: &TaskId) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn create_workflow(&self, _: &WorkflowId, _: &str) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn get_workflow(&self, id: &WorkflowId) -> Result<Option<Workflow>, BoxErr> {
        Ok(Some(Workflow {
            id: id.clone(),
            status: WorkflowStatus::Active,
            blueprint_path: String::new(),
            started_at: chrono::Utc::now(),
            blueprint_hash: None,
        }))
    }
    fn list_workflows(&self, _: Option<WorkflowStatus>) -> Result<Vec<Workflow>, BoxErr> {
        unimplemented!()
    }
    fn update_workflow_status(&self, _: &WorkflowId, _: WorkflowStatus) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn increment_worker_counter(&self, _: &WorkflowId) -> Result<usize, BoxErr> {
        unimplemented!()
    }
    fn update_blueprint_hash(&self, _: &WorkflowId, _: Option<&str>) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn dequeue_message(&self, target_id: &WorkerId) -> Result<Option<String>, BoxErr> {
        let mut messages = self.messages.lock().unwrap();
        if let Some(queue) = messages.get_mut(target_id)
            && !queue.is_empty()
        {
            return Ok(Some(queue.remove(0)));
        }
        Ok(None)
    }
}
