use super::{Task, TaskError, TaskId, TaskStatus};
use crate::review::ReviewSubmission;

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

impl<T: TaskStore> TaskStore for &T {
    type Error = T::Error;

    fn get_task(&self, id: &TaskId) -> Result<Option<Task>, Self::Error> {
        (**self).get_task(id)
    }
    fn find_reviews_for_work(&self, work_id: &TaskId) -> Result<Vec<Task>, Self::Error> {
        (**self).find_reviews_for_work(work_id)
    }
    fn find_works_for_review(&self, review_id: &TaskId) -> Result<Vec<Task>, Self::Error> {
        (**self).find_works_for_review(review_id)
    }
    fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> Result<Task, Self::Error> {
        (**self).update_task_status(id, status)
    }
    fn find_assignable_tasks(&self) -> Result<Vec<Task>, Self::Error> {
        (**self).find_assignable_tasks()
    }
    fn get_review_submissions(
        &self,
        review_id: &TaskId,
    ) -> Result<Vec<ReviewSubmission>, Self::Error> {
        (**self).get_review_submissions(review_id)
    }
}

impl<T: TaskStore> TaskStore for std::sync::Arc<T> {
    type Error = T::Error;

    fn get_task(&self, id: &TaskId) -> Result<Option<Task>, Self::Error> {
        (**self).get_task(id)
    }
    fn find_reviews_for_work(&self, work_id: &TaskId) -> Result<Vec<Task>, Self::Error> {
        (**self).find_reviews_for_work(work_id)
    }
    fn find_works_for_review(&self, review_id: &TaskId) -> Result<Vec<Task>, Self::Error> {
        (**self).find_works_for_review(review_id)
    }
    fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> Result<Task, Self::Error> {
        (**self).update_task_status(id, status)
    }
    fn find_assignable_tasks(&self) -> Result<Vec<Task>, Self::Error> {
        (**self).find_assignable_tasks()
    }
    fn get_review_submissions(
        &self,
        review_id: &TaskId,
    ) -> Result<Vec<ReviewSubmission>, Self::Error> {
        (**self).get_review_submissions(review_id)
    }
}
