use super::super::*;
use crate::models::QueuedMessage;

impl Database {
    /// Enqueue a message for a target (member or supervisor).
    pub fn enqueue_message(
        &self,
        target_id: &WorkerId,
        message: &str,
    ) -> crate::Result<QueuedMessage> {
        let conn = lock(&self.conn)?;
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        conn.execute(
            "INSERT INTO message_queue (target_id, message, created_at) VALUES (?1, ?2, ?3)",
            params![target_id.as_ref(), message, now_str],
        )?;
        let id = conn.last_insert_rowid();
        Ok(QueuedMessage {
            id,
            target_id: target_id.clone(),
            message: message.to_string(),
            created_at: now,
        })
    }
}
