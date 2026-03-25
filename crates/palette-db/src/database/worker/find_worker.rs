use super::super::{Database, lock};
use super::row::{COLUMNS, row_to_worker_state};
use palette_domain::worker::*;
use rusqlite::params;

impl Database {
    /// Find a worker by ID.
    pub fn find_worker(&self, id: &WorkerId) -> crate::Result<Option<WorkerState>> {
        let conn = lock(&self.conn)?;
        let sql = format!("SELECT {COLUMNS} FROM workers WHERE id = ?1");
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query_map(params![id.as_ref()], row_to_worker_state)?;
        rows.next().transpose().map_err(Into::into)
    }
}
