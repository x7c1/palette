use super::{Database, row_to_job};
use palette_domain::job::Job;
use palette_domain::task::TaskId;
use rusqlite::params;

impl Database {
    /// Find the job assigned to a task (if any).
    pub fn get_job_by_task_id(&self, task_id: &TaskId) -> crate::Result<Option<Job>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, task_id, type_id, title, plan_path, assignee, status_id, priority_id, repository, pr_url, created_at, updated_at, notes, assigned_at
             FROM jobs WHERE task_id = ?1",
        )?;
        let mut rows = stmt.query_map(params![task_id.as_ref()], row_to_job)?;
        rows.next().transpose().map_err(Into::into)
    }
}
