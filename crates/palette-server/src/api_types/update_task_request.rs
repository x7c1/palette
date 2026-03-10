use super::TaskStatus;
use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateTaskRequest {
    pub id: String,
    pub status: TaskStatus,
}

// TODO: Replace From with TryFrom to validate external input (see plan 009-api-input-validation)
impl From<UpdateTaskRequest> for domain::UpdateTaskRequest {
    fn from(api: UpdateTaskRequest) -> Self {
        Self {
            id: domain::TaskId::new(api.id),
            status: api.status.into(),
        }
    }
}
