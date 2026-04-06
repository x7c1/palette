use super::super::*;
use super::row::{JOB_COLUMNS, into_job, read_job_row};
use palette_domain::job::Job;
use palette_domain::task::TaskId;
use rusqlite::params;

impl Database {
    /// Find the job assigned to a task (if any).
    pub fn get_job_by_task_id(&self, task_id: &TaskId) -> crate::Result<Option<Job>> {
        let conn = lock(&self.conn)?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {JOB_COLUMNS} FROM jobs WHERE task_id = ?1"
        ))?;
        stmt.query_map(params![task_id.as_ref()], read_job_row)?
            .next()
            .transpose()?
            .map(into_job)
            .transpose()
    }
}
