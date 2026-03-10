use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SendRequest {
    pub member_id: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    pub message: String,
    /// If true, send the message without appending Enter key.
    /// Use for permission prompt responses (e.g., "2" to approve).
    #[serde(default)]
    pub no_enter: bool,
}
