use super::*;
use crate::models::StoredBlueprint;

impl Database {
    /// Get a blueprint by task_id.
    pub fn get_blueprint(&self, task_id: &str) -> crate::Result<Option<StoredBlueprint>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT task_id, title, yaml, created_at FROM blueprints WHERE task_id = ?1",
        )?;
        let mut rows = stmt.query_map(params![task_id], |row| {
            Ok(StoredBlueprint {
                task_id: row.get(0)?,
                title: row.get(1)?,
                yaml: row.get(2)?,
                created_at: parse_datetime(&row.get::<_, String>(3)?),
            })
        })?;

        match rows.next() {
            Some(Ok(bp)) => Ok(Some(bp)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }
}
