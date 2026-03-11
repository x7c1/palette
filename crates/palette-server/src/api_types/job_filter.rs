use super::JobStatus;
use super::JobType;
use palette_domain as domain;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct JobFilter {
    #[serde(rename = "type")]
    pub job_type: Option<JobType>,
    pub status: Option<JobStatus>,
    pub assignee: Option<String>,
}

impl From<JobFilter> for domain::job::JobFilter {
    fn from(api: JobFilter) -> Self {
        Self {
            job_type: api.job_type.map(domain::job::JobType::from),
            status: api.status.map(domain::job::JobStatus::from),
            assignee: api.assignee.map(domain::agent::AgentId::new),
        }
    }
}
