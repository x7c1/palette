use crate::Error;
use crate::database::Database;
use palette_domain::*;

impl TaskStore for Database {
    type Error = Error;

    fn get_task(&self, id: &TaskId) -> Result<Option<Task>, Error> {
        self.get_task(id)
    }

    fn find_reviews_for_work(&self, work_id: &TaskId) -> Result<Vec<Task>, Error> {
        self.find_reviews_for_work(work_id)
    }

    fn find_works_for_review(&self, review_id: &TaskId) -> Result<Vec<Task>, Error> {
        self.find_works_for_review(review_id)
    }

    fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> Result<Task, Error> {
        self.update_task_status(id, status)
    }

    fn find_assignable_tasks(&self) -> Result<Vec<Task>, Error> {
        self.find_assignable_tasks()
    }

    fn get_review_submissions(&self, review_id: &TaskId) -> Result<Vec<ReviewSubmission>, Error> {
        self.get_review_submissions(review_id)
    }
}
