use super::*;

impl Database {
    pub fn get_dependencies(&self, job_id: &JobId) -> crate::Result<Vec<JobId>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare("SELECT depends_on FROM dependencies WHERE job_id = ?1")?;
        let rows = stmt.query_map(params![job_id.as_ref()], |row| {
            Ok(JobId::new(row.get::<_, String>(0)?))
        })?;
        let mut deps = Vec::new();
        for row in rows {
            deps.push(row?);
        }
        Ok(deps)
    }
}
