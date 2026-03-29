use super::JobType;
use super::Priority;
use super::Repository;
use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateJobRequest {
    pub id: Option<String>,
    pub task_id: String,
    #[serde(rename = "type")]
    pub job_type: JobType,
    pub title: String,
    pub plan_path: Option<String>,
    pub assignee_id: Option<String>,
    pub priority: Option<Priority>,
    pub repository: Option<Repository>,
}

// TODO: Replace From with TryFrom to validate external input (see plan 009-api-input-validation)
impl From<CreateJobRequest> for domain::job::CreateJobRequest {
    fn from(api: CreateJobRequest) -> Self {
        Self {
            id: api.id.map(domain::job::JobId::new),
            task_id: domain::task::TaskId::new(api.task_id),
            job_type: api.job_type.into(),
            title: api.title,
            plan_path: api.plan_path,
            assignee_id: api.assignee_id.map(domain::worker::WorkerId::new),
            priority: api.priority.map(domain::job::Priority::from),
            repository: api.repository.map(Into::into),
        }
    }
}
