use super::super::*;
use crate::models::{QueuedMessage, QueuedMessageRow};
use rusqlite::OptionalExtension;

impl Database {
    /// Dequeue the next message for a target (FIFO). Returns None if empty.
    pub fn dequeue_message(&self, target_id: &WorkerId) -> crate::Result<Option<QueuedMessage>> {
        let conn = lock(&self.conn)?;
        let msg = conn
            .prepare(
                "SELECT id, target_id, message, created_at FROM message_queue WHERE target_id = ?1 ORDER BY id LIMIT 1",
            )?
            .query_row(params![target_id.as_ref()], read_queued_message_row)
            .optional()?
            .map(into_queued_message)
            .transpose()?;

        if let Some(ref msg) = msg {
            conn.execute("DELETE FROM message_queue WHERE id = ?1", params![msg.id])?;
        }
        Ok(msg)
    }
}

fn read_queued_message_row(row: &rusqlite::Row) -> rusqlite::Result<QueuedMessageRow> {
    Ok(QueuedMessageRow {
        id: row.get("id")?,
        target_id: row.get("target_id")?,
        message: row.get("message")?,
        created_at: row.get("created_at")?,
    })
}

fn into_queued_message(row: QueuedMessageRow) -> crate::Result<QueuedMessage> {
    Ok(QueuedMessage {
        id: row.id,
        target_id: WorkerId::parse(row.target_id).map_err(corrupt_parse)?,
        message: row.message,
        created_at: parse_datetime(&row.created_at),
    })
}

#[cfg(test)]
mod tests {
    use super::super::super::test_helpers::*;

    #[test]
    fn message_queue_enqueue_dequeue() {
        let db = test_db();

        // Empty queue
        assert!(db.dequeue_message(&wid("member-a")).unwrap().is_none());
        assert!(!db.has_pending_messages(&wid("member-a")).unwrap());

        // Enqueue
        let msg1 = db.enqueue_message(&wid("member-a"), "hello").unwrap();
        let msg2 = db.enqueue_message(&wid("member-a"), "world").unwrap();
        assert!(msg1.id < msg2.id);

        assert!(db.has_pending_messages(&wid("member-a")).unwrap());
        assert!(!db.has_pending_messages(&wid("member-b")).unwrap());

        // Dequeue in FIFO order
        let dequeued = db.dequeue_message(&wid("member-a")).unwrap().unwrap();
        assert_eq!(dequeued.message, "hello");

        let dequeued = db.dequeue_message(&wid("member-a")).unwrap().unwrap();
        assert_eq!(dequeued.message, "world");

        // Queue is empty
        assert!(db.dequeue_message(&wid("member-a")).unwrap().is_none());
        assert!(!db.has_pending_messages(&wid("member-a")).unwrap());
    }

    #[test]
    fn message_queue_per_target_isolation() {
        let db = test_db();

        db.enqueue_message(&wid("member-a"), "msg-a").unwrap();
        db.enqueue_message(&wid("member-b"), "msg-b").unwrap();

        let dequeued = db.dequeue_message(&wid("member-a")).unwrap().unwrap();
        assert_eq!(dequeued.message, "msg-a");

        let dequeued = db.dequeue_message(&wid("member-b")).unwrap().unwrap();
        assert_eq!(dequeued.message, "msg-b");
    }
}
