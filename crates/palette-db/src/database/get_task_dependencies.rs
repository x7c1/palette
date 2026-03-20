use super::Database;
use crate::error::Error;
use palette_domain::task::TaskId;
use rusqlite::params;

impl Database {
    /// Get all task IDs that this task depends on.
    pub fn get_task_dependencies(&self, task_id: &TaskId) -> crate::Result<Vec<TaskId>> {
        let conn = lock!(self.conn);
        let mut stmt =
            conn.prepare("SELECT depends_on FROM task_dependencies WHERE task_id = ?1")?;
        let rows = stmt.query_map(params![task_id.as_ref()], |row| {
            Ok(TaskId::new(row.get::<_, String>(0)?))
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}
