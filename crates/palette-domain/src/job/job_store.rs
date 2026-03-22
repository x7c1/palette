use super::{Job, JobError, JobId, JobStatus};
use crate::review::ReviewSubmission;

/// Abstraction over job persistence, enabling domain logic
/// to remain independent of storage implementation.
pub trait JobStore {
    type Error: From<JobError>;

    fn get_job(&self, id: &JobId) -> Result<Option<Job>, Self::Error>;
    fn update_job_status(&self, id: &JobId, status: JobStatus) -> Result<Job, Self::Error>;
    fn find_assignable_jobs(&self) -> Result<Vec<Job>, Self::Error>;
    fn get_review_submissions(
        &self,
        review_id: &JobId,
    ) -> Result<Vec<ReviewSubmission>, Self::Error>;
}

impl<T: JobStore> JobStore for &T {
    type Error = T::Error;

    fn get_job(&self, id: &JobId) -> Result<Option<Job>, Self::Error> {
        (**self).get_job(id)
    }
    fn update_job_status(&self, id: &JobId, status: JobStatus) -> Result<Job, Self::Error> {
        (**self).update_job_status(id, status)
    }
    fn find_assignable_jobs(&self) -> Result<Vec<Job>, Self::Error> {
        (**self).find_assignable_jobs()
    }
    fn get_review_submissions(
        &self,
        review_id: &JobId,
    ) -> Result<Vec<ReviewSubmission>, Self::Error> {
        (**self).get_review_submissions(review_id)
    }
}

impl<T: JobStore> JobStore for std::sync::Arc<T> {
    type Error = T::Error;

    fn get_job(&self, id: &JobId) -> Result<Option<Job>, Self::Error> {
        (**self).get_job(id)
    }
    fn update_job_status(&self, id: &JobId, status: JobStatus) -> Result<Job, Self::Error> {
        (**self).update_job_status(id, status)
    }
    fn find_assignable_jobs(&self) -> Result<Vec<Job>, Self::Error> {
        (**self).find_assignable_jobs()
    }
    fn get_review_submissions(
        &self,
        review_id: &JobId,
    ) -> Result<Vec<ReviewSubmission>, Self::Error> {
        (**self).get_review_submissions(review_id)
    }
}
