use super::super::{Database, lock};
use palette_domain::task::TaskId;
use rusqlite::params;

impl Database {
    pub fn delete_task(&self, id: &TaskId) -> crate::Result<()> {
        let conn = lock(&self.conn)?;
        conn.execute("DELETE FROM tasks WHERE id = ?1", params![id.as_ref()])?;
        Ok(())
    }
}
