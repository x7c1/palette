/// Raw database representation of a queued message record.
pub(crate) struct QueuedMessageRow {
    pub id: i64,
    pub target_id: String,
    pub message: String,
    pub created_at: String,
}
