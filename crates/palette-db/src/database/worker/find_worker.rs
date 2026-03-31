use super::super::{Database, lock};
use super::row::{COLUMNS, into_worker_state, read_worker_row};
use palette_domain::worker::*;
use rusqlite::params;

impl Database {
    /// Find a worker by ID.
    pub fn find_worker(&self, id: &WorkerId) -> crate::Result<Option<WorkerState>> {
        let conn = lock(&self.conn)?;
        let sql = format!("SELECT {COLUMNS} FROM workers WHERE id = ?1");
        let mut stmt = conn.prepare(&sql)?;
        stmt.query_map(params![id.as_ref()], read_worker_row)?
            .next()
            .transpose()?
            .map(into_worker_state)
            .transpose()
    }
}
