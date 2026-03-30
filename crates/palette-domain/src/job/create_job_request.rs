use super::{JobId, JobType, PlanPath, Priority, Repository, Title};
use crate::task::TaskId;
use crate::worker::WorkerId;

#[derive(Debug, Clone)]
pub struct CreateJobRequest {
    pub id: Option<JobId>,
    pub task_id: TaskId,
    pub job_type: JobType,
    pub title: Title,
    pub plan_path: PlanPath,
    pub assignee_id: Option<WorkerId>,
    pub priority: Option<Priority>,
    pub repository: Option<Repository>,
}
