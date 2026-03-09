use super::*;

impl Database {
    /// Find work tasks that a review task depends on.
    pub fn find_works_for_review(&self, review_id: &TaskId) -> crate::Result<Vec<Task>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT t.id, t.type, t.title, t.description, t.assignee, t.status, t.priority, t.repositories, t.pr_url, t.created_at, t.updated_at, t.notes, t.assigned_at
             FROM tasks t
             JOIN dependencies d ON d.depends_on = t.id
             WHERE d.task_id = ?1 AND t.type = 'work'",
        )?;
        let rows = stmt.query_map(params![review_id.as_ref()], |row| Ok(row_to_task(row)))?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        Ok(tasks)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;
    use palette_domain::*;

    #[test]
    fn find_works_for_review() {
        let db = test_db();
        db.create_task(&CreateTaskRequest {
            id: Some(tid("W-001")),
            task_type: TaskType::Work,
            title: "Work".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![],
        })
        .unwrap();

        db.create_task(&CreateTaskRequest {
            id: Some(tid("R-001")),
            task_type: TaskType::Review,
            title: "Review".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![tid("W-001")],
        })
        .unwrap();

        let works = db.find_works_for_review(&tid("R-001")).unwrap();
        assert_eq!(works.len(), 1);
        assert_eq!(works[0].id, tid("W-001"));
    }
}
