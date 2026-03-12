use super::*;

impl Database {
    /// Find jobs that are assignable:
    /// - Craft jobs: status = 'ready' with all craft dependencies done
    /// - Review jobs: status = 'todo' (dependency already verified by rule engine)
    ///
    /// Returns jobs ordered by priority (high > medium > low > null).
    pub fn find_assignable_jobs(&self) -> crate::Result<Vec<Job>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT t.id, t.type, t.title, t.description, t.assignee, t.status, t.priority, t.repository, t.pr_url, t.created_at, t.updated_at, t.notes, t.assigned_at
             FROM jobs t
             WHERE (
               (t.type = 'craft' AND t.status = 'ready'
                AND NOT EXISTS (
                  SELECT 1 FROM dependencies d
                  JOIN jobs dep ON d.depends_on = dep.id
                  WHERE d.job_id = t.id
                  AND dep.type = 'craft'
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
        let rows = stmt.query_map([], |row| Ok(row_to_job(row)))?;
        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row?);
        }
        Ok(jobs)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;

    use palette_domain::job::*;

    #[test]
    fn find_assignable_jobs_no_deps() {
        let db = test_db();
        create_craft(&db, "C-001", Some(Priority::High), vec![]);
        create_craft(&db, "C-002", Some(Priority::Low), vec![]);

        // Both in draft — not assignable
        assert_eq!(db.find_assignable_jobs().unwrap().len(), 0);

        // Set both to ready
        db.update_job_status(&jid("C-001"), JobStatus::Ready)
            .unwrap();
        db.update_job_status(&jid("C-002"), JobStatus::Ready)
            .unwrap();

        let assignable = db.find_assignable_jobs().unwrap();
        assert_eq!(assignable.len(), 2);
        assert_eq!(assignable[0].id, jid("C-001")); // high priority first
        assert_eq!(assignable[1].id, jid("C-002")); // low priority second
    }

    #[test]
    fn find_assignable_jobs_with_deps() {
        let db = test_db();
        create_craft(&db, "C-001", None, vec![]);
        create_craft(&db, "C-002", None, vec![jid("C-001")]);

        db.update_job_status(&jid("C-001"), JobStatus::Ready)
            .unwrap();
        db.update_job_status(&jid("C-002"), JobStatus::Ready)
            .unwrap();

        // C-002 depends on C-001 which is not done
        let assignable = db.find_assignable_jobs().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, jid("C-001"));

        // Complete C-001
        db.update_job_status(&jid("C-001"), JobStatus::InProgress)
            .unwrap();
        db.update_job_status(&jid("C-001"), JobStatus::InReview)
            .unwrap();
        db.update_job_status(&jid("C-001"), JobStatus::Done)
            .unwrap();

        // Now C-002 is assignable
        let assignable = db.find_assignable_jobs().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, jid("C-002"));
    }

    #[test]
    fn find_assignable_jobs_review_todo() {
        let db = test_db();
        create_craft(&db, "C-001", None, vec![]);
        create_review(&db, "R-001", vec![jid("C-001")]);

        // Review starts as Todo (initial status for review jobs)
        let assignable = db.find_assignable_jobs().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, jid("R-001"));

        // Set craft to ready — both are now assignable
        db.update_job_status(&jid("C-001"), JobStatus::Ready)
            .unwrap();
        let assignable = db.find_assignable_jobs().unwrap();
        assert_eq!(assignable.len(), 2);

        // Assign review — only craft remains assignable
        db.assign_job(&jid("R-001"), &aid("m-r")).unwrap();
        let assignable = db.find_assignable_jobs().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, jid("C-001"));
    }

    #[test]
    fn find_assignable_jobs_diamond_dag() {
        let db = test_db();
        create_craft(&db, "A", None, vec![]);
        create_craft(&db, "B", None, vec![jid("A")]);
        create_craft(&db, "C", None, vec![jid("A")]);
        create_craft(&db, "D", None, vec![jid("B"), jid("C")]);

        for id in ["A", "B", "C", "D"] {
            db.update_job_status(&jid(id), JobStatus::Ready).unwrap();
        }

        let assignable = db.find_assignable_jobs().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, jid("A"));

        db.assign_job(&jid("A"), &aid("m-a")).unwrap();
        db.update_job_status(&jid("A"), JobStatus::InReview)
            .unwrap();
        db.update_job_status(&jid("A"), JobStatus::Done).unwrap();

        let assignable = db.find_assignable_jobs().unwrap();
        assert_eq!(assignable.len(), 2);
        let ids: Vec<&str> = assignable.iter().map(|j| j.id.as_ref()).collect();
        assert!(ids.contains(&"B"));
        assert!(ids.contains(&"C"));

        db.assign_job(&jid("B"), &aid("m-b")).unwrap();
        db.update_job_status(&jid("B"), JobStatus::InReview)
            .unwrap();
        db.update_job_status(&jid("B"), JobStatus::Done).unwrap();

        let assignable = db.find_assignable_jobs().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, jid("C"));

        db.assign_job(&jid("C"), &aid("m-c")).unwrap();
        db.update_job_status(&jid("C"), JobStatus::InReview)
            .unwrap();
        db.update_job_status(&jid("C"), JobStatus::Done).unwrap();

        let assignable = db.find_assignable_jobs().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, jid("D"));
    }
}
