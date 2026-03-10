use super::*;

impl Database {
    /// Find review tasks that depend on the given work task.
    pub fn find_reviews_for_work(&self, work_id: &TaskId) -> crate::Result<Vec<Task>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT t.id, t.type, t.title, t.description, t.assignee, t.status, t.priority, t.repositories, t.pr_url, t.created_at, t.updated_at, t.notes, t.assigned_at
             FROM tasks t
             JOIN dependencies d ON d.task_id = t.id
             WHERE d.depends_on = ?1 AND t.type = 'review'",
        )?;
        let rows = stmt.query_map(params![work_id.as_ref()], |row| Ok(row_to_task(row)))?;
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

    use palette_domain::task::*;

    #[test]
    fn find_reviews_for_work() {
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

        let reviews = db.find_reviews_for_work(&tid("W-001")).unwrap();
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].id, tid("R-001"));
    }
}
