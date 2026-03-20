use super::JobType;
use super::Priority;
use super::Repository;
use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateJobRequest {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub job_type: JobType,
    pub title: String,
    pub plan_path: String,
    pub description: Option<String>,
    pub assignee: Option<String>,
    pub priority: Option<Priority>,
    pub repository: Option<Repository>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

// TODO: Replace From with TryFrom to validate external input (see plan 009-api-input-validation)
impl From<CreateJobRequest> for domain::job::CreateJobRequest {
    fn from(api: CreateJobRequest) -> Self {
        Self {
            id: api.id.map(domain::job::JobId::new),
            task_id: None,
            job_type: api.job_type.into(),
            title: api.title,
            plan_path: api.plan_path,
            description: api.description,
            assignee: api.assignee.map(domain::agent::AgentId::new),
            priority: api.priority.map(domain::job::Priority::from),
            repository: api.repository.map(Into::into),
            depends_on: api
                .depends_on
                .into_iter()
                .map(domain::job::JobId::new)
                .collect(),
        }
    }
}
