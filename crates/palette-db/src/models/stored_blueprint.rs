use chrono::{DateTime, Utc};
use palette_domain::blueprint::Blueprint;

/// A Blueprint row in the database.
#[derive(Debug, Clone)]
pub(crate) struct StoredBlueprint {
    pub task_id: String,
    pub title: String,
    pub yaml: String,
    pub created_at: DateTime<Utc>,
}

impl From<StoredBlueprint> for Blueprint {
    fn from(bp: StoredBlueprint) -> Self {
        Self {
            task_id: bp.task_id,
            title: bp.title,
            yaml: bp.yaml,
            created_at: bp.created_at,
        }
    }
}
