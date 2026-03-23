use palette_domain::job::JobType;

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
