/// Raw database representation of a worker record.
pub(crate) struct WorkerRow {
    pub id: String,
    pub workflow_id: String,
    pub role_id: i64,
    pub status_id: i64,
    pub supervisor_id: Option<String>,
    pub container_id: String,
    pub terminal_target: String,
    pub session_id: Option<String>,
    pub task_id: String,
}
