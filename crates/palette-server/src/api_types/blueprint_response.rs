use chrono::{DateTime, Utc};
use palette_db::models::StoredBlueprint;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct BlueprintResponse {
    pub task_id: String,
    pub title: String,
    pub yaml: String,
    pub created_at: DateTime<Utc>,
}

impl From<StoredBlueprint> for BlueprintResponse {
    fn from(bp: StoredBlueprint) -> Self {
        Self {
            task_id: bp.task_id,
            title: bp.title,
            yaml: bp.yaml,
            created_at: bp.created_at,
        }
    }
}
