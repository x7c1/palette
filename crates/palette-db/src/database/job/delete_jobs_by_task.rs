use super::super::{Database, lock};
use palette_domain::task::TaskId;
use rusqlite::params;

impl Database {
    pub fn delete_jobs_by_task_id(&self, task_id: &TaskId) -> crate::Result<()> {
        let conn = lock(&self.conn)?;
        conn.execute(
            "DELETE FROM jobs WHERE task_id = ?1",
            params![task_id.as_ref()],
        )?;
        Ok(())
    }
}
