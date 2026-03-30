use super::{FieldError, JobType, Priority, Repository};
use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateJobRequest {
    pub id: Option<String>,
    pub task_id: String,
    #[serde(rename = "type")]
    pub job_type: JobType,
    pub title: String,
    pub plan_path: String,
    pub assignee_id: Option<String>,
    pub priority: Option<Priority>,
    pub repository: Option<Repository>,
}

impl CreateJobRequest {
    pub fn validate(&self) -> Result<domain::job::CreateJobRequest, Vec<FieldError>> {
        palette_macros::validate!(domain::job::CreateJobRequest::new {
            id: self
                .id
                .as_deref()
                .map(domain::job::JobId::parse)
                .transpose(),
            task_id: domain::task::TaskId::parse(&self.task_id),
            #[plain]
            job_type: self.job_type.into(),
            title: domain::job::Title::parse(&self.title),
            plan_path: domain::job::PlanPath::parse(&self.plan_path),
            #[plain]
            assignee_id: self
                .assignee_id
                .as_deref()
                .map(domain::worker::WorkerId::new),
            #[plain]
            priority: self.priority.map(domain::job::Priority::from),
            #[plain]
            repository: self.repository.clone().map(Into::into),
        })
    }
}
