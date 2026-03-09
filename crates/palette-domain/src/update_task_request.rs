use crate::task_id::TaskId;
use crate::task_status::TaskStatus;

#[derive(Debug, Clone)]
pub struct UpdateTaskRequest {
    pub id: TaskId,
    pub status: TaskStatus,
}
