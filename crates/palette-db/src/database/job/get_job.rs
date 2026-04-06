use super::super::*;
use super::row::{JOB_COLUMNS, into_job, read_job_row};

impl Database {
    pub fn get_job(&self, id: &JobId) -> crate::Result<Option<Job>> {
        let conn = lock(&self.conn)?;
        let mut stmt = conn.prepare(&format!("SELECT {JOB_COLUMNS} FROM jobs WHERE id = ?1"))?;
        stmt.query_map(params![id.as_ref()], read_job_row)?
            .next()
            .transpose()?
            .map(into_job)
            .transpose()
    }
}
