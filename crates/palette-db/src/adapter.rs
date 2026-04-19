use crate::Database;
use palette_domain::job::{CreateJobRequest, Job, JobFilter, JobId, JobStatus, JobType};
use palette_domain::review::{ReviewComment, ReviewSubmission, SubmitReviewRequest};
use palette_domain::task::{TaskId, TaskState, TaskStatus};
use palette_domain::worker::{ContainerId, WorkerId, WorkerSessionId, WorkerState, WorkerStatus};
use palette_domain::workflow::{Workflow, WorkflowId, WorkflowStatus};
use palette_usecase::{CreateTaskRequest, DataStore, InsertWorkerRequest};
use std::collections::HashMap;

impl DataStore for Database {
    // -- Worker --

    fn insert_worker(
        &self,
        req: &InsertWorkerRequest,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let db_req = crate::InsertWorkerRequest {
            id: req.id.clone(),
            workflow_id: req.workflow_id.clone(),
            role: req.role,
            status: req.status,
            supervisor_id: req.supervisor_id.clone(),
            container_id: req.container_id.clone(),
            terminal_target: req.terminal_target.clone(),
            session_id: req.session_id.clone(),
            task_id: req.task_id.clone(),
        };
        Ok(self.insert_worker(&db_req)?)
    }

    fn find_worker(
        &self,
        id: &WorkerId,
    ) -> Result<Option<WorkerState>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.find_worker(id)?)
    }

    fn find_worker_by_container(
        &self,
        container_id: &ContainerId,
    ) -> Result<Option<WorkerState>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.find_worker_by_container(container_id)?)
    }

    fn list_supervisors(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<WorkerState>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.list_supervisors(workflow_id)?)
    }

    fn list_members(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<Vec<WorkerState>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.list_members(workflow_id)?)
    }

    fn list_all_workers(
        &self,
    ) -> Result<Vec<WorkerState>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::list_all_workers(self)?)
    }

    fn list_booting_workers(
        &self,
    ) -> Result<Vec<WorkerState>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::list_booting_workers(self)?)
    }

    fn list_idle_or_waiting_workers(
        &self,
    ) -> Result<Vec<WorkerState>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::list_idle_or_waiting_workers(self)?)
    }

    fn update_worker_status(
        &self,
        id: &WorkerId,
        status: WorkerStatus,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::update_worker_status(self, id, status)?)
    }

    fn update_worker_session_id(
        &self,
        id: &WorkerId,
        session_id: &WorkerSessionId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::update_worker_session_id(self, id, session_id)?)
    }

    fn remove_worker(
        &self,
        id: &WorkerId,
    ) -> Result<Option<WorkerState>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::remove_worker(self, id)?)
    }

    fn find_supervisor_for_task(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<WorkerState>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::find_supervisor_for_task(self, task_id)?)
    }

    fn find_supervisors_for_task(
        &self,
        task_id: &TaskId,
    ) -> Result<Vec<WorkerState>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::find_supervisors_for_task(self, task_id)?)
    }

    // -- Job --

    fn create_job(
        &self,
        req: &CreateJobRequest,
    ) -> Result<Job, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::create_job(self, req)?)
    }

    fn get_job(&self, id: &JobId) -> Result<Option<Job>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::get_job(self, id)?)
    }

    fn get_job_by_task_id(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<Job>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::get_job_by_task_id(self, task_id)?)
    }

    fn list_jobs(
        &self,
        filter: &JobFilter,
    ) -> Result<Vec<Job>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::list_jobs(self, filter)?)
    }

    fn assign_job(
        &self,
        job_id: &JobId,
        assignee_id: &WorkerId,
        job_type: JobType,
    ) -> Result<Job, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::assign_job(self, job_id, assignee_id, job_type)?)
    }

    fn update_job_status(
        &self,
        id: &JobId,
        status: JobStatus,
    ) -> Result<Job, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::update_job_status(self, id, status)?)
    }

    fn find_assignable_jobs(&self) -> Result<Vec<Job>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::find_assignable_jobs(self)?)
    }

    fn count_active_workers(&self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::count_active_workers(self)?)
    }

    fn submit_review(
        &self,
        review_job_id: &JobId,
        req: &SubmitReviewRequest,
    ) -> Result<ReviewSubmission, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::submit_review(self, review_job_id, req)?)
    }

    fn get_review_submissions(
        &self,
        review_job_id: &JobId,
    ) -> Result<Vec<ReviewSubmission>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::get_review_submissions(self, review_job_id)?)
    }

    fn get_review_comments(
        &self,
        submission_id: i64,
    ) -> Result<Vec<ReviewComment>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::get_review_comments(self, submission_id)?)
    }

    // -- Task --

    fn create_task(
        &self,
        req: &CreateTaskRequest,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let db_req = crate::CreateTaskRequest {
            id: req.id.clone(),
            workflow_id: req.workflow_id.clone(),
        };
        Ok(Database::create_task(self, &db_req)?)
    }

    fn get_task_state(
        &self,
        id: &TaskId,
    ) -> Result<Option<TaskState>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::get_task_state(self, id)?)
    }

    fn get_task_statuses(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<HashMap<TaskId, TaskStatus>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::get_task_statuses(self, workflow_id)?)
    }

    fn update_task_status(
        &self,
        id: &TaskId,
        status: TaskStatus,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::update_task_status(self, id, status)?)
    }

    fn delete_task(&self, id: &TaskId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::delete_task(self, id)?)
    }

    fn delete_jobs_by_task_id(
        &self,
        task_id: &TaskId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::delete_jobs_by_task_id(self, task_id)?)
    }

    fn delete_review_data_by_workflow(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<(usize, usize), Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::delete_review_data_by_workflow(self, workflow_id)?)
    }

    // -- Workflow --

    fn create_workflow(
        &self,
        id: &WorkflowId,
        blueprint_path: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::create_workflow(self, id, blueprint_path)?)
    }

    fn find_workflow(
        &self,
        id: &WorkflowId,
    ) -> Result<Option<Workflow>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::get_workflow(self, id)?)
    }

    fn list_workflows(
        &self,
        status: Option<WorkflowStatus>,
    ) -> Result<Vec<Workflow>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::list_workflows(self, status)?)
    }

    fn update_workflow_status(
        &self,
        id: &WorkflowId,
        status: WorkflowStatus,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::update_workflow_status(self, id, status)?)
    }

    fn mark_workflow_failed(
        &self,
        id: &WorkflowId,
        reason: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::mark_workflow_failed(self, id, reason)?)
    }

    fn find_active_workflows_using_branch(
        &self,
        repo_name: &str,
        branch: &str,
    ) -> Result<Vec<WorkflowId>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::find_active_workflows_using_branch(
            self, repo_name, branch,
        )?)
    }

    fn increment_worker_counter(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::increment_worker_counter(self, workflow_id)?)
    }

    fn update_blueprint_hash(
        &self,
        id: &WorkflowId,
        hash: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::update_blueprint_hash(self, id, hash)?)
    }

    fn delete_workflow(
        &self,
        id: &WorkflowId,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::delete_workflow(self, id)?)
    }

    // -- Message Queue --

    fn enqueue_message(
        &self,
        target_id: &WorkerId,
        message: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Database::enqueue_message(self, target_id, message)?;
        Ok(())
    }

    fn dequeue_message(
        &self,
        target_id: &WorkerId,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::dequeue_message(self, target_id)?.map(|m| m.message))
    }

    fn has_pending_messages(
        &self,
        target_id: &WorkerId,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::has_pending_messages(self, target_id)?)
    }

    fn delete_messages_by_targets(
        &self,
        target_ids: &[WorkerId],
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Database::delete_messages_by_targets(self, target_ids)?)
    }
}
