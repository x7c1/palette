use super::{FieldHint, JobStatus};
use palette_domain as domain;
use serde::{Deserialize, Serialize};

const MAX_ID_LEN: usize = 256;

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateJobRequest {
    pub id: String,
    pub status: JobStatus,
}

impl UpdateJobRequest {
    pub fn validate_id(&self) -> Result<domain::job::JobId, Vec<FieldHint>> {
        if self.id.trim().is_empty() {
            return Err(vec![FieldHint {
                field: "id".into(),
                reason: "required".into(),
            }]);
        }
        if self.id.len() > MAX_ID_LEN {
            return Err(vec![FieldHint {
                field: "id".into(),
                reason: "too_long".into(),
            }]);
        }
        Ok(domain::job::JobId::new(&self.id))
    }
}
