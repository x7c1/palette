/// Raw database representation of a job record.
/// All fields use DB-native types (String, i64, Option<String>).
pub(crate) struct JobRow {
    pub id: String,
    pub task_id: String,
    pub type_id: i64,
    pub title: String,
    pub plan_path: Option<String>,
    pub assignee_id: Option<String>,
    pub status_id: i64,
    pub priority_id: Option<i64>,
    pub repository: Option<String>,
    pub command: Option<String>,
    pub perspective: Option<String>,
    pub pull_request: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub assigned_at: Option<String>,
}
