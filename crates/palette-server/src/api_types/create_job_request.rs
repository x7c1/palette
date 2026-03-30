use super::{FieldError, JobType, Priority, Repository};
use palette_domain as domain;
use palette_domain::ReasonKey;
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
        let mut hints = Vec::new();

        if let Err(e) = domain::job::Title::parse(&self.title) {
            hints.push(FieldError {
                field: "title".into(),
                reason: e.reason_key(),
            });
        }

        if let Err(e) = domain::task::TaskId::parse(&self.task_id) {
            hints.push(FieldError {
                field: "task_id".into(),
                reason: e.reason_key(),
            });
        }

        if let Err(e) = domain::job::PlanPath::parse(&self.plan_path) {
            hints.push(FieldError {
                field: "plan_path".into(),
                reason: e.reason_key(),
            });
        }

        if let Some(ref id) = self.id
            && let Err(e) = domain::job::JobId::parse(id)
        {
            hints.push(FieldError {
                field: "id".into(),
                reason: e.reason_key(),
            });
        }

        if !hints.is_empty() {
            return Err(hints);
        }

        // All validations passed — parse again to obtain the values.
        // These are pure and cheap; duplicating the call avoids
        // carrying Options that would require expect/unwrap.
        let task_id = domain::task::TaskId::parse(&self.task_id).map_err(|e| {
            vec![FieldError {
                field: "task_id".into(),
                reason: e.reason_key(),
            }]
        })?;

        let title = domain::job::Title::parse(&self.title).map_err(|e| {
            vec![FieldError {
                field: "title".into(),
                reason: e.reason_key(),
            }]
        })?;

        let plan_path = domain::job::PlanPath::parse(&self.plan_path).map_err(|e| {
            vec![FieldError {
                field: "plan_path".into(),
                reason: e.reason_key(),
            }]
        })?;

        let id = self
            .id
            .as_deref()
            .map(domain::job::JobId::parse)
            .transpose()
            .map_err(|e| {
                vec![FieldError {
                    field: "id".into(),
                    reason: e.reason_key(),
                }]
            })?;

        Ok(domain::job::CreateJobRequest {
            id,
            task_id,
            job_type: self.job_type.into(),
            title,
            plan_path,
            assignee_id: self
                .assignee_id
                .as_deref()
                .map(domain::worker::WorkerId::new),
            priority: self.priority.map(domain::job::Priority::from),
            repository: self.repository.clone().map(Into::into),
        })
    }
}
