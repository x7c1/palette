use super::{JobId, JobStatus};

#[derive(Debug, Clone)]
pub struct UpdateJobRequest {
    pub id: JobId,
    pub status: JobStatus,
}
