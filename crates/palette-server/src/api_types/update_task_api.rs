use super::TaskStatusApi;
use palette_domain::{TaskId, UpdateTaskRequest};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct UpdateTaskApi {
    pub id: String,
    pub status: TaskStatusApi,
}

impl From<UpdateTaskApi> for UpdateTaskRequest {
    fn from(api: UpdateTaskApi) -> Self {
        Self {
            id: TaskId::new(api.id),
            status: api.status.into(),
        }
    }
}
