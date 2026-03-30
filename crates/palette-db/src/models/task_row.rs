/// Raw database representation of a task record.
pub(crate) struct TaskRow {
    pub id: String,
    pub workflow_id: String,
    pub status_id: i64,
}
