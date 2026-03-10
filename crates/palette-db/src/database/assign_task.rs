use super::*;

impl Database {
    /// Assign a task to a member and set status to in_progress.
    pub fn assign_task(&self, task_id: &TaskId, assignee: &AgentId) -> crate::Result<Task> {
        let conn = lock!(self.conn);
        let now = Utc::now().to_rfc3339();
        let updated = conn.execute(
            "UPDATE tasks SET status = ?1, assignee = ?2, assigned_at = ?3, updated_at = ?4 WHERE id = ?5",
            params![TaskStatus::InProgress.as_str(), assignee.as_ref(), now, now, task_id.as_ref()],
        )?;
        if updated == 0 {
            return Err(TaskError::NotFound {
                task_id: task_id.clone(),
            }
            .into());
        }
        drop(conn);
        self.get_task(task_id)?.ok_or_else(|| {
            TaskError::NotFound {
                task_id: task_id.clone(),
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
    fn assign_task_sets_assignee_and_status() {
        let db = test_db();
        create_work(&db, "W-001", None, vec![]);
        db.update_task_status(&tid("W-001"), TaskStatus::Ready)
            .unwrap();

        let task = db.assign_task(&tid("W-001"), &aid("member-a")).unwrap();
        assert_eq!(task.status, TaskStatus::InProgress);
        assert_eq!(task.assignee, Some(aid("member-a")));
        assert!(task.assigned_at.is_some());
    }
}
