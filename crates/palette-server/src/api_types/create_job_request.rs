use super::{FieldError, JobType, Priority, Repository};
use palette_domain::job::{CreateJobRequest as DomainCreateJobRequest, JobId, PlanPath, Title};
use palette_domain::task::TaskId;
use palette_domain::worker::WorkerId;
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
    pub fn validate(&self) -> Result<DomainCreateJobRequest, Vec<FieldError>> {
        palette_macros::validate!(DomainCreateJobRequest::new {
            id: self.id.as_deref().map(JobId::parse).transpose(),
            task_id: TaskId::parse(&self.task_id),
            #[plain]
            job_type: self.job_type.into(),
            title: Title::parse(&self.title),
            plan_path: PlanPath::parse(&self.plan_path),
            #[plain]
            assignee_id: self.assignee_id.as_deref().map(WorkerId::new),
            #[plain]
            priority: self.priority.map(palette_domain::job::Priority::from),
            #[plain]
            repository: self.repository.clone().map(Into::into),
        })
    }
}
