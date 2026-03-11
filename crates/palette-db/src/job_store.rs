use crate::Error;
use crate::database::Database;
use palette_domain::job::*;
use palette_domain::review::*;

impl JobStore for Database {
    type Error = Error;

    fn get_job(&self, id: &JobId) -> Result<Option<Job>, Error> {
        self.get_job(id)
    }

    fn find_reviews_for_craft(&self, craft_id: &JobId) -> Result<Vec<Job>, Error> {
        self.find_reviews_for_craft(craft_id)
    }

    fn find_crafts_for_review(&self, review_id: &JobId) -> Result<Vec<Job>, Error> {
        self.find_crafts_for_review(review_id)
    }

    fn update_job_status(&self, id: &JobId, status: JobStatus) -> Result<Job, Error> {
        self.update_job_status(id, status)
    }

    fn find_assignable_jobs(&self) -> Result<Vec<Job>, Error> {
        self.find_assignable_jobs()
    }

    fn get_review_submissions(&self, review_id: &JobId) -> Result<Vec<ReviewSubmission>, Error> {
        self.get_review_submissions(review_id)
    }
}
