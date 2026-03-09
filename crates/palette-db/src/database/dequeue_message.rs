use super::*;
use crate::models::QueuedMessage;

impl Database {
    /// Dequeue the next message for a target (FIFO). Returns None if empty.
    pub fn dequeue_message(&self, target_id: &AgentId) -> Result<Option<QueuedMessage>, DbError> {
        let conn = lock!(self.conn);
        let msg = conn
            .prepare(
                "SELECT id, target_id, message, created_at FROM message_queue WHERE target_id = ?1 ORDER BY id LIMIT 1",
            )?
            .query_row(params![target_id.as_ref()], |row| {
                Ok(QueuedMessage {
                    id: row.get(0)?,
                    target_id: AgentId::new(row.get::<_, String>(1)?),
                    message: row.get(2)?,
                    created_at: parse_datetime(&row.get::<_, String>(3)?),
                })
            })
            .ok();

        if let Some(ref msg) = msg {
            conn.execute("DELETE FROM message_queue WHERE id = ?1", params![msg.id])?;
        }
        Ok(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;

    #[test]
    fn message_queue_enqueue_dequeue() {
        let db = test_db();

        // Empty queue
        assert!(db.dequeue_message(&aid("member-a")).unwrap().is_none());
        assert!(!db.has_pending_messages(&aid("member-a")).unwrap());

        // Enqueue
        let msg1 = db.enqueue_message(&aid("member-a"), "hello").unwrap();
        let msg2 = db.enqueue_message(&aid("member-a"), "world").unwrap();
        assert!(msg1.id < msg2.id);

        assert!(db.has_pending_messages(&aid("member-a")).unwrap());
        assert!(!db.has_pending_messages(&aid("member-b")).unwrap());

        // Dequeue in FIFO order
        let dequeued = db.dequeue_message(&aid("member-a")).unwrap().unwrap();
        assert_eq!(dequeued.message, "hello");

        let dequeued = db.dequeue_message(&aid("member-a")).unwrap().unwrap();
        assert_eq!(dequeued.message, "world");

        // Queue is empty
        assert!(db.dequeue_message(&aid("member-a")).unwrap().is_none());
        assert!(!db.has_pending_messages(&aid("member-a")).unwrap());
    }

    #[test]
    fn message_queue_per_target_isolation() {
        let db = test_db();

        db.enqueue_message(&aid("member-a"), "msg-a").unwrap();
        db.enqueue_message(&aid("member-b"), "msg-b").unwrap();

        let dequeued = db.dequeue_message(&aid("member-a")).unwrap().unwrap();
        assert_eq!(dequeued.message, "msg-a");

        let dequeued = db.dequeue_message(&aid("member-b")).unwrap().unwrap();
        assert_eq!(dequeued.message, "msg-b");
    }
}
