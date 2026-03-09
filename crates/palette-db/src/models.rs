use chrono::{DateTime, Utc};
use palette_domain::AgentId;
use serde::{Deserialize, Serialize};

/// A queued message in the message_queue table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedMessage {
    pub id: i64,
    pub target_id: AgentId,
    pub message: String,
    pub created_at: DateTime<Utc>,
}
