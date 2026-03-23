use palette_domain::job::{JobStatus, JobType};

use super::{craft_status_from_id, craft_status_id, review_status_from_id, review_status_id};

pub fn job_status_id(status: JobStatus) -> i64 {
    match status {
        JobStatus::Craft(s) => craft_status_id(s),
        JobStatus::Review(s) => review_status_id(s),
    }
}

pub fn job_status_from_id(id: i64, job_type: JobType) -> Result<JobStatus, String> {
    match job_type {
        JobType::Craft => craft_status_from_id(id).map(JobStatus::Craft),
        JobType::Review => review_status_from_id(id).map(JobStatus::Review),
    }
}
