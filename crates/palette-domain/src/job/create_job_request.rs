use super::{JobDetail, PlanPath, Priority, Title};
use crate::task::TaskId;
use crate::worker::WorkerId;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CreateJobRequest {
    pub task_id: TaskId,
    pub title: Title,
    pub plan_path: Option<PlanPath>,
    pub assignee_id: Option<WorkerId>,
    pub priority: Option<Priority>,
    pub detail: JobDetail,
}

impl CreateJobRequest {
    pub fn new(
        task_id: TaskId,
        title: Title,
        plan_path: Option<PlanPath>,
        assignee_id: Option<WorkerId>,
        priority: Option<Priority>,
        detail: JobDetail,
    ) -> Self {
        Self {
            task_id,
            title,
            plan_path,
            assignee_id,
            priority,
            detail,
        }
    }
}
