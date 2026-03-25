use super::super::*;

impl Database {
    /// Check if a target has pending messages.
    pub fn has_pending_messages(&self, target_id: &WorkerId) -> crate::Result<bool> {
        let conn = lock!(self.conn);
        let exists = conn
            .prepare("SELECT 1 FROM message_queue WHERE target_id = ?1 LIMIT 1")?
            .exists(params![target_id.as_ref()])?;
        Ok(exists)
    }
}
