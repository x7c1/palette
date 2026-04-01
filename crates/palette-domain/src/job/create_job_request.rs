use super::{JobId, JobType, PlanPath, Priority, Repository, Title};
use crate::task::TaskId;
use crate::worker::WorkerId;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CreateJobRequest {
    pub id: Option<JobId>,
    pub task_id: TaskId,
    pub job_type: JobType,
    pub title: Title,
    pub plan_path: PlanPath,
    pub assignee_id: Option<WorkerId>,
    pub priority: Option<Priority>,
    pub repository: Option<Repository>,
    /// Command for orchestrator tasks.
    pub command: Option<String>,
}

impl CreateJobRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Option<JobId>,
        task_id: TaskId,
        job_type: JobType,
        title: Title,
        plan_path: PlanPath,
        assignee_id: Option<WorkerId>,
        priority: Option<Priority>,
        repository: Option<Repository>,
        command: Option<String>,
    ) -> Self {
        Self {
            id,
            task_id,
            job_type,
            title,
            plan_path,
            assignee_id,
            priority,
            repository,
            command,
        }
    }
}
