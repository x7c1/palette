//! Mapping between domain enums and their integer IDs in lookup tables.
//!
//! These IDs correspond to the seed data in schema.rs.

use palette_domain::job::{CraftStatus, JobStatus, JobType, ReviewStatus};
use palette_domain::review::Verdict;
use palette_domain::task::TaskStatus;
use palette_domain::workflow::WorkflowStatus;

// -- JobType --

pub fn job_type_id(job_type: JobType) -> i64 {
    match job_type {
        JobType::Craft => 1,
        JobType::Review => 2,
    }
}

pub fn job_type_from_id(id: i64) -> Result<JobType, String> {
    match id {
        1 => Ok(JobType::Craft),
        2 => Ok(JobType::Review),
        _ => Err(format!("invalid job_type id: {id}")),
    }
}

// -- CraftStatus --

pub fn craft_status_id(status: CraftStatus) -> i64 {
    match status {
        CraftStatus::Todo => 1,
        CraftStatus::InProgress => 2,
        CraftStatus::InReview => 3,
        CraftStatus::Done => 4,
        CraftStatus::Escalated => 5,
    }
}

pub fn craft_status_from_id(id: i64) -> Result<CraftStatus, String> {
    match id {
        1 => Ok(CraftStatus::Todo),
        2 => Ok(CraftStatus::InProgress),
        3 => Ok(CraftStatus::InReview),
        4 => Ok(CraftStatus::Done),
        5 => Ok(CraftStatus::Escalated),
        _ => Err(format!("invalid craft_status id: {id}")),
    }
}

// -- ReviewStatus --

pub fn review_status_id(status: ReviewStatus) -> i64 {
    match status {
        ReviewStatus::Todo => 6,
        ReviewStatus::InProgress => 7,
        ReviewStatus::ChangesRequested => 8,
        ReviewStatus::Done => 9,
        ReviewStatus::Escalated => 10,
    }
}

pub fn review_status_from_id(id: i64) -> Result<ReviewStatus, String> {
    match id {
        6 => Ok(ReviewStatus::Todo),
        7 => Ok(ReviewStatus::InProgress),
        8 => Ok(ReviewStatus::ChangesRequested),
        9 => Ok(ReviewStatus::Done),
        10 => Ok(ReviewStatus::Escalated),
        _ => Err(format!("invalid review_status id: {id}")),
    }
}

// -- JobStatus (composite) --

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

// -- TaskStatus --

pub fn task_status_id(status: TaskStatus) -> i64 {
    match status {
        TaskStatus::Pending => 1,
        TaskStatus::Ready => 2,
        TaskStatus::InProgress => 3,
        TaskStatus::Suspended => 4,
        TaskStatus::Completed => 5,
    }
}

pub fn task_status_from_id(id: i64) -> Result<TaskStatus, String> {
    match id {
        1 => Ok(TaskStatus::Pending),
        2 => Ok(TaskStatus::Ready),
        3 => Ok(TaskStatus::InProgress),
        4 => Ok(TaskStatus::Suspended),
        5 => Ok(TaskStatus::Completed),
        _ => Err(format!("invalid task_status id: {id}")),
    }
}

// -- WorkflowStatus --

pub fn workflow_status_id(status: WorkflowStatus) -> i64 {
    match status {
        WorkflowStatus::Active => 1,
        WorkflowStatus::Suspended => 2,
        WorkflowStatus::Completed => 3,
    }
}

pub fn workflow_status_from_id(id: i64) -> Result<WorkflowStatus, String> {
    match id {
        1 => Ok(WorkflowStatus::Active),
        2 => Ok(WorkflowStatus::Suspended),
        3 => Ok(WorkflowStatus::Completed),
        _ => Err(format!("invalid workflow_status id: {id}")),
    }
}

// -- Verdict --

pub fn verdict_id(verdict: Verdict) -> i64 {
    match verdict {
        Verdict::Approved => 1,
        Verdict::ChangesRequested => 2,
    }
}

pub fn verdict_from_id(id: i64) -> Result<Verdict, String> {
    match id {
        1 => Ok(Verdict::Approved),
        2 => Ok(Verdict::ChangesRequested),
        _ => Err(format!("invalid verdict id: {id}")),
    }
}
