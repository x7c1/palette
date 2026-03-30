use super::{FieldError, JobStatus};
use palette_domain as domain;
use palette_domain::ReasonKey;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateJobRequest {
    pub id: String,
    pub status: JobStatus,
}

impl UpdateJobRequest {
    pub fn validate_id(&self) -> Result<domain::job::JobId, Vec<FieldError>> {
        domain::job::JobId::parse(&self.id).map_err(|e| {
            vec![FieldError {
                field: "id".into(),
                reason: e.reason_key(),
            }]
        })
    }
}
