use super::{Task, TaskId, TaskStatus};

/// Abstraction over task persistence, enabling domain logic
/// to remain independent of storage implementation.
pub trait TaskStore {
    type Error: std::fmt::Debug;

    fn get_task(&self, id: &TaskId) -> Result<Option<Task>, Self::Error>;
    fn get_child_tasks(&self, parent_id: &TaskId) -> Result<Vec<Task>, Self::Error>;
    fn get_task_dependencies(&self, task_id: &TaskId) -> Result<Vec<TaskId>, Self::Error>;
    fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> Result<(), Self::Error>;
}
