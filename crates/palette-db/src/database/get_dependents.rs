use super::*;

impl Database {
    pub fn get_dependents(&self, depends_on_id: &JobId) -> crate::Result<Vec<JobId>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare("SELECT job_id FROM dependencies WHERE depends_on = ?1")?;
        let rows = stmt.query_map(params![depends_on_id.as_ref()], |row| {
            Ok(JobId::new(row.get::<_, String>(0)?))
        })?;
        let mut deps = Vec::new();
        for row in rows {
            deps.push(row?);
        }
        Ok(deps)
    }
}
