#[cfg(test)]
mod tests {
    use crate::Database;
    use palette_domain::rule::{TaskEffect, TaskRuleEngine};
    use palette_domain::task::{TaskId, TaskStatus};
    use palette_domain::workflow::WorkflowId;

    use crate::database::CreateTaskRequest;

    fn setup() -> (Database, WorkflowId) {
        let db = Database::open_in_memory().unwrap();
        let wf_id = WorkflowId::new("wf-test");
        db.create_workflow(&wf_id, "test").unwrap();
        (db, wf_id)
    }

    fn add_task(
        db: &Database,
        wf_id: &WorkflowId,
        id: &str,
        parent_id: Option<&str>,
        status: TaskStatus,
        deps: Vec<&str>,
    ) {
        db.create_task(&CreateTaskRequest {
            id: TaskId::new(id),
            workflow_id: wf_id.clone(),
            parent_id: parent_id.map(TaskId::new),
            title: id.to_string(),
            plan_path: None,
            job_type: None,
            depends_on: deps.into_iter().map(TaskId::new).collect(),
        })
        .unwrap();
        if status != TaskStatus::Pending {
            db.update_task_status(&TaskId::new(id), status).unwrap();
        }
    }

    #[test]
    fn tasks_without_deps_become_ready() {
        let (db, wf_id) = setup();
        add_task(&db, &wf_id, "root", None, TaskStatus::InProgress, vec![]);
        add_task(&db, &wf_id, "a", Some("root"), TaskStatus::Pending, vec![]);
        add_task(
            &db,
            &wf_id,
            "b",
            Some("root"),
            TaskStatus::Pending,
            vec!["a"],
        );

        let engine = TaskRuleEngine::new(&db);
        let task_ids = vec![TaskId::new("a"), TaskId::new("b")];
        let effects = engine.resolve_ready_tasks(&task_ids).unwrap();

        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            TaskEffect::TaskStatusChanged {
                task_id: TaskId::new("a"),
                new_status: TaskStatus::Ready,
            }
        );
    }

    #[test]
    fn completing_task_unblocks_dependents() {
        let (db, wf_id) = setup();
        add_task(&db, &wf_id, "root", None, TaskStatus::InProgress, vec![]);
        add_task(&db, &wf_id, "a", Some("root"), TaskStatus::Done, vec![]);
        add_task(
            &db,
            &wf_id,
            "b",
            Some("root"),
            TaskStatus::Pending,
            vec!["a"],
        );

        let engine = TaskRuleEngine::new(&db);
        let effects = engine.on_task_completed(&TaskId::new("a")).unwrap();

        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            TaskEffect::TaskStatusChanged {
                task_id: TaskId::new("b"),
                new_status: TaskStatus::Ready,
            }
        );
    }

    #[test]
    fn all_children_done_completes_parent() {
        let (db, wf_id) = setup();
        add_task(&db, &wf_id, "root", None, TaskStatus::InProgress, vec![]);
        add_task(&db, &wf_id, "a", Some("root"), TaskStatus::Done, vec![]);
        add_task(&db, &wf_id, "b", Some("root"), TaskStatus::Done, vec![]);

        let engine = TaskRuleEngine::new(&db);
        let effects = engine.on_task_completed(&TaskId::new("b")).unwrap();

        assert!(effects.iter().any(|e| *e
            == TaskEffect::TaskStatusChanged {
                task_id: TaskId::new("root"),
                new_status: TaskStatus::Done,
            }));
    }
}
