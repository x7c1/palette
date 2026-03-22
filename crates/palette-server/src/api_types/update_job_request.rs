use super::JobStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateJobRequest {
    pub id: String,
    pub status: JobStatus,
}
