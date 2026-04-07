/// Raw database representation of a workflow record.
pub(crate) struct WorkflowRow {
    pub id: String,
    pub blueprint_path: Option<String>,
    pub status_id: i64,
    pub started_at: String,
    pub blueprint_hash: Option<String>,
}
