use serde::{Deserialize, Serialize};

/// Identifies the Task that a Blueprint belongs to.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Task {
    /// Task identifier (slash-separated hierarchy, e.g. "2026/feature-x").
    pub id: String,
    pub title: String,
}
