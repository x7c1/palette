use palette_domain::job::{JobStatus, JobType};

use super::{
    craft_status_from_id, craft_status_id, mechanized_status_from_id, mechanized_status_id,
    review_status_from_id, review_status_id,
};

pub fn job_status_id(status: JobStatus) -> i64 {
    match status {
        JobStatus::Craft(s) => craft_status_id(s),
        JobStatus::Review(s) => review_status_id(s),
        // Orchestrator statuses: DB IDs 11-14 (offset 10)
        JobStatus::Orchestrator(s) => mechanized_status_id(s) + 10,
        // Operator statuses: DB IDs 15-18 (offset 14)
        JobStatus::Operator(s) => mechanized_status_id(s) + 14,
    }
}

pub fn job_status_from_id(id: i64, job_type: JobType) -> Result<JobStatus, String> {
    match job_type {
        JobType::Craft => craft_status_from_id(id).map(JobStatus::Craft),
        JobType::Review => review_status_from_id(id).map(JobStatus::Review),
        // Orchestrator statuses: DB IDs 11-14 → MechanizedStatus IDs 1-4
        JobType::Orchestrator => mechanized_status_from_id(id - 10).map(JobStatus::Orchestrator),
        // Operator statuses: DB IDs 15-18 → MechanizedStatus IDs 1-4
        JobType::Operator => mechanized_status_from_id(id - 14).map(JobStatus::Operator),
    }
}
