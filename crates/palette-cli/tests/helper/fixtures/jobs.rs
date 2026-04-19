use palette_server::api_types::{CreateJobRequest, JobStatus, JobType, UpdateJobRequest};

pub fn create_craft(id: &str, title: &str, task_id: &str) -> CreateJobRequest {
    CreateJobRequest {
        task_id: task_id.to_string(),
        job_type: JobType::Craft,
        title: title.to_string(),
        plan_path: Some(format!("test/{id}")),
        assignee_id: None,
        priority: None,
        repository: Some(palette_server::api_types::Repository {
            name: "x7c1/palette-demo".to_string(),
            work_branch: "main".to_string(),
            source_branch: None,
        }),
        command: None,
    }
}

pub fn create_review(id: &str, title: &str, task_id: &str) -> CreateJobRequest {
    CreateJobRequest {
        task_id: task_id.to_string(),
        job_type: JobType::Review,
        title: title.to_string(),
        plan_path: Some(format!("test/{id}")),
        assignee_id: None,
        priority: None,
        repository: None,
        command: None,
    }
}

pub fn update_status(id: &str, status: JobStatus) -> UpdateJobRequest {
    UpdateJobRequest {
        id: id.to_string(),
        status,
    }
}
