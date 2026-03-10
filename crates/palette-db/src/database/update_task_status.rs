use super::*;

impl Database {
    pub fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> crate::Result<Task> {
        let conn = lock!(self.conn);
        let now = Utc::now().to_rfc3339();
        let updated = conn.execute(
            "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status.as_str(), now, id.as_ref()],
        )?;
        if updated == 0 {
            return Err(TaskError::NotFound {
                task_id: id.clone(),
            }
            .into());
        }
        drop(conn);
        self.get_task(id)?.ok_or_else(|| {
            TaskError::NotFound {
                task_id: id.clone(),
            }
            .into()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;

    use palette_domain::task::*;

    #[test]
    fn update_task_status() {
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

        let updated = db
            .update_task_status(&tid("W-001"), TaskStatus::InProgress)
            .unwrap();
        assert_eq!(updated.status, TaskStatus::InProgress);
    }
}
