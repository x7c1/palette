use super::{JobId, JobType, Priority, Repository};
use crate::task::TaskId;
use crate::worker::WorkerId;

#[derive(Debug, Clone)]
pub struct CreateJobRequest {
    pub id: Option<JobId>,
    pub task_id: TaskId,
    pub job_type: JobType,
    pub title: String,
    pub plan_path: String,
    pub assignee: Option<WorkerId>,
    pub priority: Option<Priority>,
    pub repository: Option<Repository>,
}
