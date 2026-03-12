use chrono::{DateTime, Utc};
use palette_domain::blueprint::Blueprint;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct BlueprintResponse {
    pub task_id: String,
    pub title: String,
    pub yaml: String,
    pub created_at: DateTime<Utc>,
}

impl From<Blueprint> for BlueprintResponse {
    fn from(bp: Blueprint) -> Self {
        Self {
            task_id: bp.task_id,
            title: bp.title,
            yaml: bp.yaml,
            created_at: bp.created_at,
        }
    }
}
