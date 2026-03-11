use chrono::{DateTime, Utc};

/// A Blueprint stored in the database.
#[derive(Debug, Clone)]
pub struct StoredBlueprint {
    pub task_id: String,
    pub title: String,
    pub yaml: String,
    pub created_at: DateTime<Utc>,
}
