use super::super::*;
use palette_domain::worker::WorkerId;
use rusqlite::ToSql;

impl Database {
    pub fn delete_messages_by_targets(&self, target_ids: &[WorkerId]) -> crate::Result<usize> {
        if target_ids.is_empty() {
            return Ok(0);
        }
        let conn = lock(&self.conn)?;
        let placeholders = std::iter::repeat_n("?", target_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!("DELETE FROM message_queue WHERE target_id IN ({placeholders})");
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<String> = target_ids.iter().map(|w| w.as_ref().to_string()).collect();
        let params_ref: Vec<&dyn ToSql> = params.iter().map(|p| p as &dyn ToSql).collect();
        let deleted = stmt.execute(params_ref.as_slice())?;
        Ok(deleted)
    }
}
