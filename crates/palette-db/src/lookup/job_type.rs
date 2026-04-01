use palette_domain::job::JobType;

pub fn job_type_id(job_type: JobType) -> i64 {
    match job_type {
        JobType::Craft => 1,
        JobType::Review => 2,
        JobType::ReviewIntegrate => 5,
        JobType::Orchestrator => 3,
        JobType::Operator => 4,
    }
}

pub fn job_type_from_id(id: i64) -> Result<JobType, String> {
    match id {
        1 => Ok(JobType::Craft),
        2 => Ok(JobType::Review),
        5 => Ok(JobType::ReviewIntegrate),
        3 => Ok(JobType::Orchestrator),
        4 => Ok(JobType::Operator),
        _ => Err(format!("invalid job_type id: {id}")),
    }
}
