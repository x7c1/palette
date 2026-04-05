use super::{JobDetail, JobId, PlanPath, Priority, Title};
use crate::task::TaskId;
use crate::worker::WorkerId;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CreateJobRequest {
    pub id: Option<JobId>,
    pub task_id: TaskId,
    pub title: Title,
    pub plan_path: PlanPath,
    pub assignee_id: Option<WorkerId>,
    pub priority: Option<Priority>,
    pub detail: JobDetail,
}

impl CreateJobRequest {
    pub fn new(
        id: Option<JobId>,
        task_id: TaskId,
        title: Title,
        plan_path: PlanPath,
        assignee_id: Option<WorkerId>,
        priority: Option<Priority>,
        detail: JobDetail,
    ) -> Self {
        Self {
            id,
            task_id,
            title,
            plan_path,
            assignee_id,
            priority,
            detail,
        }
    }
}
