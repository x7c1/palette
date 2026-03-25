use chrono::{DateTime, Utc};
use palette_domain::worker::WorkerId;

/// A queued message in the message_queue table.
#[derive(Debug, Clone)]
pub struct QueuedMessage {
    pub id: i64,
    pub target_id: WorkerId,
    pub message: String,
    pub created_at: DateTime<Utc>,
}
