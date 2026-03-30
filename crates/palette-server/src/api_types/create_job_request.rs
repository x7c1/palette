use super::{FieldHint, JobType, Priority, Repository};
use palette_domain as domain;
use serde::{Deserialize, Serialize};

const MAX_TITLE_LEN: usize = 500;
const MAX_ID_LEN: usize = 256;
const MAX_PATH_LEN: usize = 1024;

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
    pub fn validate(&self) -> Result<domain::job::CreateJobRequest, Vec<FieldHint>> {
        let mut hints = Vec::new();

        if self.title.trim().is_empty() {
            hints.push(FieldHint {
                field: "title".into(),
                reason: "required".into(),
            });
        } else if self.title.len() > MAX_TITLE_LEN {
            hints.push(FieldHint {
                field: "title".into(),
                reason: "too_long".into(),
            });
        }

        if let Err(e) = domain::task::TaskId::parse(&self.task_id) {
            hints.push(FieldHint {
                field: "task_id".into(),
                reason: e.reason_key().into(),
            });
        }

        if self.plan_path.trim().is_empty() {
            hints.push(FieldHint {
                field: "plan_path".into(),
                reason: "required".into(),
            });
        } else if self.plan_path.len() > MAX_PATH_LEN {
            hints.push(FieldHint {
                field: "plan_path".into(),
                reason: "too_long".into(),
            });
        }

        if let Some(ref id) = self.id {
            if id.trim().is_empty() {
                hints.push(FieldHint {
                    field: "id".into(),
                    reason: "required".into(),
                });
            } else if id.len() > MAX_ID_LEN {
                hints.push(FieldHint {
                    field: "id".into(),
                    reason: "too_long".into(),
                });
            }
        }

        if !hints.is_empty() {
            return Err(hints);
        }

        // All validations passed — parse again to obtain the value.
        // TaskId::parse is pure and cheap; duplicating the call avoids
        // carrying an Option that would require expect/unwrap.
        let task_id = domain::task::TaskId::parse(&self.task_id).map_err(|e| {
            vec![FieldHint {
                field: "task_id".into(),
                reason: e.reason_key().into(),
            }]
        })?;

        Ok(domain::job::CreateJobRequest {
            id: self.id.as_deref().map(domain::job::JobId::new),
            task_id,
            job_type: self.job_type.into(),
            title: self.title.clone(),
            plan_path: self.plan_path.clone(),
            assignee_id: self
                .assignee_id
                .as_deref()
                .map(domain::worker::WorkerId::new),
            priority: self.priority.map(domain::job::Priority::from),
            repository: self.repository.clone().map(Into::into),
        })
    }
}
