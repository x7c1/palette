use palette_domain::task::TaskId;
use palette_domain::worker::WorkerId;
use palette_domain::workflow::WorkflowId;
use std::path::PathBuf;

#[derive(Default)]
pub struct AdminDeletedCounts {
    pub workflows: usize,
    pub tasks: usize,
    pub jobs: usize,
    pub workers: usize,
    pub review_submissions: usize,
    pub review_comments: usize,
    pub message_queue: usize,
}

pub struct AdminCleanupPlan {
    pub workflow_ids: Vec<WorkflowId>,
    pub task_ids: Vec<TaskId>,
    pub job_ids: Vec<String>,
    pub worker_ids: Vec<WorkerId>,
    pub file_paths: Vec<PathBuf>,
}
