use super::super::*;
use palette_domain::workflow::WorkflowId;

impl Database {
    pub fn delete_workflow(&self, id: &WorkflowId) -> crate::Result<usize> {
        let conn = lock(&self.conn)?;
        let deleted = conn.execute("DELETE FROM workflows WHERE id = ?1", [id.as_ref()])?;
        Ok(deleted)
    }
}
