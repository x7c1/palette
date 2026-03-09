use crate::review_submission::ReviewSubmission;
use crate::task::Task;
use crate::task_error::TaskError;
use crate::task_id::TaskId;
use crate::task_status::TaskStatus;

/// Abstraction over task persistence, enabling domain logic
/// to remain independent of storage implementation.
pub trait TaskStore {
    type Error: From<TaskError>;

    fn get_task(&self, id: &TaskId) -> Result<Option<Task>, Self::Error>;
    fn find_reviews_for_work(&self, work_id: &TaskId) -> Result<Vec<Task>, Self::Error>;
    fn find_works_for_review(&self, review_id: &TaskId) -> Result<Vec<Task>, Self::Error>;
    fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> Result<Task, Self::Error>;
    fn find_assignable_tasks(&self) -> Result<Vec<Task>, Self::Error>;
    fn get_review_submissions(
        &self,
        review_id: &TaskId,
    ) -> Result<Vec<ReviewSubmission>, Self::Error>;
}
