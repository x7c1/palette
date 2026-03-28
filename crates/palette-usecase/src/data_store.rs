use palette_domain::job::{CreateJobRequest, Job, JobFilter, JobId, JobStatus, JobType};
use palette_domain::review::{ReviewComment, ReviewSubmission, SubmitReviewRequest};
use palette_domain::task::{TaskId, TaskState, TaskStatus};
use palette_domain::terminal::TerminalTarget;
use palette_domain::worker::{
    ContainerId, WorkerId, WorkerRole, WorkerSessionId, WorkerState, WorkerStatus,
};
use palette_domain::workflow::{Workflow, WorkflowId, WorkflowStatus};
use std::collections::HashMap;

/// Request to insert a new worker into the data store.
pub struct InsertWorkerRequest {
    pub id: WorkerId,
    pub workflow_id: WorkflowId,
    pub role: WorkerRole,
    pub status: WorkerStatus,
    pub supervisor_id: WorkerId,
    pub container_id: ContainerId,
    pub terminal_target: TerminalTarget,
    pub session_id: Option<WorkerSessionId>,
    pub task_id: TaskId,
}

/// Request to create a new task in the data store.
pub struct CreateTaskRequest {
    pub id: TaskId,
    pub workflow_id: WorkflowId,
}

/// Port for data persistence.
///
/// Abstracts database operations so that the orchestrator and server
/// can be tested with mock implementations.
pub trait DataStore: Send + Sync {
    // -- Worker --

    fn insert_worker(
        &self,
        req: &InsertWorkerRequest,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn find_worker(
        &self,
        id: &WorkerId,
    ) -> Result<Option<WorkerState>, Box<dyn std::error::Error + Send + Sync>>;

    fn find_worker_by_container(
        &self,
        container_id: &ContainerId,
    ) -> Result<Option<WorkerState>, Box<dyn std::error::Error + Send + Sync>>;

    fn list_supervisors(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<WorkerState>, Box<dyn std::error::Error + Send + Sync>>;

    fn list_members(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<WorkerState>, Box<dyn std::error::Error + Send + Sync>>;

    fn list_all_workers(
        &self,
    ) -> Result<Vec<WorkerState>, Box<dyn std::error::Error + Send + Sync>>;

    fn list_booting_workers(
        &self,
    ) -> Result<Vec<WorkerState>, Box<dyn std::error::Error + Send + Sync>>;

    fn list_idle_or_waiting_workers(
        &self,
    ) -> Result<Vec<WorkerState>, Box<dyn std::error::Error + Send + Sync>>;

    fn update_worker_status(
        &self,
        id: &WorkerId,
        status: WorkerStatus,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn update_worker_session_id(
        &self,
        id: &WorkerId,
        session_id: &WorkerSessionId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn remove_worker(
        &self,
        id: &WorkerId,
    ) -> Result<Option<WorkerState>, Box<dyn std::error::Error + Send + Sync>>;

    fn find_supervisor_for_task(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<WorkerState>, Box<dyn std::error::Error + Send + Sync>>;

    // -- Job --

    fn create_job(
        &self,
        req: &CreateJobRequest,
    ) -> Result<Job, Box<dyn std::error::Error + Send + Sync>>;

    fn get_job(&self, id: &JobId) -> Result<Option<Job>, Box<dyn std::error::Error + Send + Sync>>;

    fn get_job_by_task_id(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<Job>, Box<dyn std::error::Error + Send + Sync>>;

    fn list_jobs(
        &self,
        filter: &JobFilter,
    ) -> Result<Vec<Job>, Box<dyn std::error::Error + Send + Sync>>;

    fn assign_job(
        &self,
        job_id: &JobId,
        assignee_id: &WorkerId,
        job_type: JobType,
    ) -> Result<Job, Box<dyn std::error::Error + Send + Sync>>;

    fn update_job_status(
        &self,
        id: &JobId,
        status: JobStatus,
    ) -> Result<Job, Box<dyn std::error::Error + Send + Sync>>;

    fn find_assignable_jobs(&self) -> Result<Vec<Job>, Box<dyn std::error::Error + Send + Sync>>;

    fn count_active_workers(&self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>>;

    fn submit_review(
        &self,
        review_job_id: &JobId,
        req: &SubmitReviewRequest,
    ) -> Result<ReviewSubmission, Box<dyn std::error::Error + Send + Sync>>;

    fn get_review_submissions(
        &self,
        review_job_id: &JobId,
    ) -> Result<Vec<ReviewSubmission>, Box<dyn std::error::Error + Send + Sync>>;

    fn get_review_comments(
        &self,
        submission_id: i64,
    ) -> Result<Vec<ReviewComment>, Box<dyn std::error::Error + Send + Sync>>;

    // -- Task --

    fn create_task(
        &self,
        req: &CreateTaskRequest,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn get_task_state(
        &self,
        id: &TaskId,
    ) -> Result<Option<TaskState>, Box<dyn std::error::Error + Send + Sync>>;

    fn get_task_statuses(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<HashMap<TaskId, TaskStatus>, Box<dyn std::error::Error + Send + Sync>>;

    fn update_task_status(
        &self,
        id: &TaskId,
        status: TaskStatus,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    // -- Workflow --

    fn create_workflow(
        &self,
        id: &WorkflowId,
        blueprint_path: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn get_workflow(
        &self,
        id: &WorkflowId,
    ) -> Result<Option<Workflow>, Box<dyn std::error::Error + Send + Sync>>;

    fn list_workflows(
        &self,
        status: Option<WorkflowStatus>,
    ) -> Result<Vec<Workflow>, Box<dyn std::error::Error + Send + Sync>>;

    fn update_workflow_status(
        &self,
        id: &WorkflowId,
        status: WorkflowStatus,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn increment_worker_counter(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>>;

    // -- Message Queue --

    fn enqueue_message(
        &self,
        target_id: &WorkerId,
        message: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn dequeue_message(
        &self,
        target_id: &WorkerId,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>>;

    fn has_pending_messages(
        &self,
        target_id: &WorkerId,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;
}
