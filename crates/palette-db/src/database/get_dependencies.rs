use super::*;

impl Database {
    pub fn get_dependencies(&self, task_id: &TaskId) -> crate::Result<Vec<TaskId>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare("SELECT depends_on FROM dependencies WHERE task_id = ?1")?;
        let rows = stmt.query_map(params![task_id.as_ref()], |row| {
            Ok(TaskId::new(row.get::<_, String>(0)?))
        })?;
        let mut deps = Vec::new();
        for row in rows {
            deps.push(row?);
        }
        Ok(deps)
    }
}
