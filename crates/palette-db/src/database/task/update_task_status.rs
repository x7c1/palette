use super::super::{Database, lock};
use palette_domain::task::{TaskId, TaskStatus};
use rusqlite::params;

impl Database {
    pub fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> crate::Result<()> {
        let conn = lock(&self.conn)?;
        conn.execute(
            "UPDATE tasks SET status_id = ?1 WHERE id = ?2",
            params![crate::lookup::task_status_id(status), id.as_ref()],
        )?;
        Ok(())
    }
}
