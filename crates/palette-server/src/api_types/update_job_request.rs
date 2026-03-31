use super::{InputError, JobStatus, Location};
use palette_core::ReasonKey;
use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateJobRequest {
    pub id: String,
    pub status: JobStatus,
}

impl UpdateJobRequest {
    pub fn validate_id(&self) -> Result<domain::job::JobId, Vec<InputError>> {
        domain::job::JobId::parse(&self.id).map_err(|e| {
            vec![InputError {
                location: Location::Body,
                hint: "id".into(),
                reason: e.reason_key(),
            }]
        })
    }
}
