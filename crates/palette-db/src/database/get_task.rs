use super::*;

impl Database {
    pub fn get_task(&self, id: &TaskId) -> Result<Option<Task>, DbError> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, type, title, description, assignee, status, priority, repositories, pr_url, created_at, updated_at, notes, assigned_at
             FROM tasks WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id.as_ref()], |row| Ok(row_to_task(row)))?;
        match rows.next() {
            Some(Ok(task)) => Ok(Some(task)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }
}
