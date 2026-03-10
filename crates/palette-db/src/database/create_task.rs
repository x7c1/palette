use super::*;

impl Database {
    pub fn create_task(&self, req: &CreateTaskRequest) -> crate::Result<Task> {
        let mut conn = lock!(self.conn);
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let id = req
            .id
            .clone()
            .unwrap_or_else(|| TaskId::generate(req.task_type));

        let repos_json = req
            .repositories
            .as_ref()
            .map(|r| repository_row::repositories_to_json(r));

        // Work tasks start as Draft; review tasks start as Todo
        let initial_status = match req.task_type {
            TaskType::Work => TaskStatus::Draft,
            TaskType::Review => TaskStatus::Todo,
        };

        let tx = conn.transaction()?;

        tx.execute(
            "INSERT INTO tasks (id, type, title, description, assignee, status, priority, repositories, pr_url, created_at, updated_at, notes, assigned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?10, NULL, NULL)",
            params![
                id.as_ref(),
                req.task_type.as_str(),
                req.title,
                req.description,
                req.assignee.as_ref().map(|a| a.as_ref()),
                initial_status.as_str(),
                req.priority.map(|p| p.as_str()),
                repos_json,
                now_str,
                now_str,
            ],
        )?;

        for dep in &req.depends_on {
            tx.execute(
                "INSERT INTO dependencies (task_id, depends_on) VALUES (?1, ?2)",
                params![id.as_ref(), dep.as_ref()],
            )?;
        }

        let task = query_task(&tx, &id)?
            .ok_or_else(|| Error::Task(TaskError::NotFound { task_id: id }))?;

        tx.commit()?;
        Ok(task)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;

    use palette_domain::task::*;

    #[test]
    fn create_and_get_task() {
        let db = test_db();
        let task = db
            .create_task(&CreateTaskRequest {
                id: Some(tid("W-001")),
                task_type: TaskType::Work,
                title: "Implement feature".to_string(),
                description: Some("Details".to_string()),
                assignee: Some(aid("member-a")),
                priority: Some(Priority::High),
                repositories: Some(vec![Repository {
                    name: "x7c1/palette".to_string(),
                    branch: Some("feature/test".to_string()),
                }]),
                depends_on: vec![],
            })
            .unwrap();

        assert_eq!(task.id, tid("W-001"));
        assert_eq!(task.task_type, TaskType::Work);
        assert_eq!(task.status, TaskStatus::Draft);
        assert_eq!(task.priority, Some(Priority::High));

        let fetched = db.get_task(&tid("W-001")).unwrap().unwrap();
        assert_eq!(fetched.title, "Implement feature");
    }

    #[test]
    fn create_task_with_dependencies() {
        let db = test_db();
        db.create_task(&CreateTaskRequest {
            id: Some(tid("W-001")),
            task_type: TaskType::Work,
            title: "Work task".to_string(),
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
            title: "Review task".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![tid("W-001")],
        })
        .unwrap();

        let deps = db.get_dependencies(&tid("R-001")).unwrap();
        assert_eq!(deps, vec![tid("W-001")]);

        let dependents = db.get_dependents(&tid("W-001")).unwrap();
        assert_eq!(dependents, vec![tid("R-001")]);
    }
}
