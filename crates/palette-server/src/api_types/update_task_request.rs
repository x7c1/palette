use super::TaskStatus;
use palette_domain as domain;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    pub id: String,
    pub status: TaskStatus,
}

impl From<UpdateTaskRequest> for domain::UpdateTaskRequest {
    fn from(api: UpdateTaskRequest) -> Self {
        Self {
            id: domain::TaskId::new(api.id),
            status: api.status.into(),
        }
    }
}
