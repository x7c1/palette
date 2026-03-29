use super::super::*;
use super::row::row_to_job;

impl Database {
    pub fn get_job(&self, id: &JobId) -> crate::Result<Option<Job>> {
        let conn = lock(&self.conn)?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, type_id, title, plan_path, assignee_id, status_id, priority_id, repository, pr_url, created_at, updated_at, notes, assigned_at
             FROM jobs WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id.as_ref()], row_to_job)?;
        rows.next().transpose().map_err(Into::into)
    }
}
