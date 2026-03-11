use super::JobStatus;
use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateJobRequest {
    pub id: String,
    pub status: JobStatus,
}

// TODO: Replace From with TryFrom to validate external input (see plan 009-api-input-validation)
impl From<UpdateJobRequest> for domain::job::UpdateJobRequest {
    fn from(api: UpdateJobRequest) -> Self {
        Self {
            id: domain::job::JobId::new(api.id),
            status: api.status.into(),
        }
    }
}
