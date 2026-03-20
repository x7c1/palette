use super::Database;
use crate::error::Error;
use palette_domain::task::{TaskId, TaskStatus};
use rusqlite::params;

impl Database {
    pub fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> crate::Result<()> {
        let conn = lock!(self.conn);
        conn.execute(
            "UPDATE tasks SET status = ?1 WHERE id = ?2",
            params![status.as_str(), id.as_ref()],
        )?;
        Ok(())
    }
}
