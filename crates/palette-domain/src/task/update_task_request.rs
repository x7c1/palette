use super::{TaskId, TaskStatus};

#[derive(Debug, Clone)]
pub struct UpdateTaskRequest {
    pub id: TaskId,
    pub status: TaskStatus,
}
