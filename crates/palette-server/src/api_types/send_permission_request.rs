use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SendPermissionRequest {
    pub worker_id: String,
    pub event_id: String,
    pub choice: String,
}
