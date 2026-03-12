use super::*;

impl Database {
    pub fn get_job(&self, id: &JobId) -> crate::Result<Option<Job>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, type, title, description, assignee, status, priority, repository, pr_url, created_at, updated_at, notes, assigned_at
             FROM jobs WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id.as_ref()], |row| Ok(row_to_job(row)))?;
        match rows.next() {
            Some(Ok(job)) => Ok(Some(job)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }
}
