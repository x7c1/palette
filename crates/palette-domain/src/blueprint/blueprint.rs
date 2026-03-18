use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Blueprint {
    pub task_id: String,
    pub title: String,
    pub yaml: String,
    pub created_at: DateTime<Utc>,
}
