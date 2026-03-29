use super::super::{Database, lock};
use palette_domain::workflow::WorkflowId;
use rusqlite::params;

impl Database {
    pub fn update_blueprint_hash(&self, id: &WorkflowId, hash: Option<&str>) -> crate::Result<()> {
        let conn = lock(&self.conn)?;
        conn.execute(
            "UPDATE workflows SET blueprint_hash = ?1 WHERE id = ?2",
            params![hash, id.as_ref()],
        )?;
        Ok(())
    }
}
