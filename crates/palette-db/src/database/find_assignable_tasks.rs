use super::*;

impl Database {
    /// Find tasks that are assignable:
    /// - Work tasks: status = 'ready' with all work dependencies done
    /// - Review tasks: status = 'todo' (dependency already verified by rule engine)
    /// Returns tasks ordered by priority (high > medium > low > null).
    pub fn find_assignable_tasks(&self) -> crate::Result<Vec<Task>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT t.id, t.type, t.title, t.description, t.assignee, t.status, t.priority, t.repositories, t.pr_url, t.created_at, t.updated_at, t.notes, t.assigned_at
             FROM tasks t
             WHERE (
               (t.type = 'work' AND t.status = 'ready'
                AND NOT EXISTS (
                  SELECT 1 FROM dependencies d
                  JOIN tasks dep ON d.depends_on = dep.id
                  WHERE d.task_id = t.id
                  AND dep.type = 'work'
                  AND dep.status != 'done'
                ))
               OR
               (t.type = 'review' AND t.status = 'todo' AND t.assignee IS NULL)
             )
             ORDER BY
               CASE t.priority
                 WHEN 'high' THEN 0
                 WHEN 'medium' THEN 1
                 WHEN 'low' THEN 2
                 ELSE 3
               END",
        )?;
        let rows = stmt.query_map([], |row| Ok(row_to_task(row)))?;
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
    fn find_assignable_tasks_no_deps() {
        let db = test_db();
        create_work(&db, "W-001", Some(Priority::High), vec![]);
        create_work(&db, "W-002", Some(Priority::Low), vec![]);

        // Both in draft — not assignable
        assert_eq!(db.find_assignable_tasks().unwrap().len(), 0);

        // Set both to ready
        db.update_task_status(&tid("W-001"), TaskStatus::Ready)
            .unwrap();
        db.update_task_status(&tid("W-002"), TaskStatus::Ready)
            .unwrap();

        let assignable = db.find_assignable_tasks().unwrap();
        assert_eq!(assignable.len(), 2);
        assert_eq!(assignable[0].id, tid("W-001")); // high priority first
        assert_eq!(assignable[1].id, tid("W-002")); // low priority second
    }

    #[test]
    fn find_assignable_tasks_with_deps() {
        let db = test_db();
        create_work(&db, "W-001", None, vec![]);
        create_work(&db, "W-002", None, vec![tid("W-001")]);

        db.update_task_status(&tid("W-001"), TaskStatus::Ready)
            .unwrap();
        db.update_task_status(&tid("W-002"), TaskStatus::Ready)
            .unwrap();

        // W-002 depends on W-001 which is not done
        let assignable = db.find_assignable_tasks().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, tid("W-001"));

        // Complete W-001
        db.update_task_status(&tid("W-001"), TaskStatus::InProgress)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::InReview)
            .unwrap();
        db.update_task_status(&tid("W-001"), TaskStatus::Done)
            .unwrap();

        // Now W-002 is assignable
        let assignable = db.find_assignable_tasks().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, tid("W-002"));
    }

    #[test]
    fn find_assignable_tasks_review_todo() {
        let db = test_db();
        create_work(&db, "W-001", None, vec![]);
        create_review(&db, "R-001", vec![tid("W-001")]);

        // Review starts as Todo (initial status for review tasks)
        // Both work (draft) and review (todo, unassigned) are in the DB
        // Work is not ready yet, but review is todo and unassigned → assignable
        let assignable = db.find_assignable_tasks().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, tid("R-001"));

        // Set work to ready — both are now assignable
        db.update_task_status(&tid("W-001"), TaskStatus::Ready)
            .unwrap();
        let assignable = db.find_assignable_tasks().unwrap();
        assert_eq!(assignable.len(), 2);

        // Assign review — only work remains assignable
        db.assign_task(&tid("R-001"), &aid("m-r")).unwrap();
        let assignable = db.find_assignable_tasks().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, tid("W-001"));
    }

    #[test]
    fn find_assignable_tasks_diamond_dag() {
        let db = test_db();
        //   A
        //  / \
        // B   C
        //  \ /
        //   D
        create_work(&db, "A", None, vec![]);
        create_work(&db, "B", None, vec![tid("A")]);
        create_work(&db, "C", None, vec![tid("A")]);
        create_work(&db, "D", None, vec![tid("B"), tid("C")]);

        for id in ["A", "B", "C", "D"] {
            db.update_task_status(&tid(id), TaskStatus::Ready).unwrap();
        }

        // Only A is assignable
        let assignable = db.find_assignable_tasks().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, tid("A"));

        // Complete A → B and C become assignable
        db.assign_task(&tid("A"), &aid("m-a")).unwrap();
        db.update_task_status(&tid("A"), TaskStatus::InReview)
            .unwrap();
        db.update_task_status(&tid("A"), TaskStatus::Done).unwrap();

        let assignable = db.find_assignable_tasks().unwrap();
        assert_eq!(assignable.len(), 2);
        let ids: Vec<&str> = assignable.iter().map(|t| t.id.as_ref()).collect();
        assert!(ids.contains(&"B"));
        assert!(ids.contains(&"C"));

        // Complete B, but D still waits for C
        db.assign_task(&tid("B"), &aid("m-b")).unwrap();
        db.update_task_status(&tid("B"), TaskStatus::InReview)
            .unwrap();
        db.update_task_status(&tid("B"), TaskStatus::Done).unwrap();

        let assignable = db.find_assignable_tasks().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, tid("C"));

        // Complete C → D becomes assignable
        db.assign_task(&tid("C"), &aid("m-c")).unwrap();
        db.update_task_status(&tid("C"), TaskStatus::InReview)
            .unwrap();
        db.update_task_status(&tid("C"), TaskStatus::Done).unwrap();

        let assignable = db.find_assignable_tasks().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, tid("D"));
    }
}
