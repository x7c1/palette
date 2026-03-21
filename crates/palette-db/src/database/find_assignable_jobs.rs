use super::*;

impl Database {
    /// Find jobs that are assignable:
    /// - Craft jobs: status = 'ready' with no assignee
    /// - Review jobs: status = 'todo' with no assignee
    ///
    /// Dependencies are now managed at the Task level by TaskRuleEngine,
    /// so jobs only reach 'ready'/'todo' when their task dependencies are satisfied.
    ///
    /// Returns jobs ordered by priority (high > medium > low > null).
    pub fn find_assignable_jobs(&self) -> crate::Result<Vec<Job>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT t.id, t.task_id, t.type, t.title, t.plan_path, t.description, t.assignee, t.status, t.priority, t.repository, t.pr_url, t.created_at, t.updated_at, t.notes, t.assigned_at
             FROM jobs t
             WHERE (
               (t.type = 'craft' AND t.status = 'ready' AND t.assignee IS NULL)
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
        let rows = stmt.query_map([], row_to_job)?;
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
    fn find_assignable_craft_jobs() {
        let db = test_db();
        create_craft(&db, "C-001", Some(Priority::High));
        create_craft(&db, "C-002", Some(Priority::Low));

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

        // Assign one — only the other remains assignable
        db.assign_job(&jid("C-001"), &aid("m-a")).unwrap();
        let assignable = db.find_assignable_jobs().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, jid("C-002"));
    }

    #[test]
    fn find_assignable_review_jobs() {
        let db = test_db();
        create_review(&db, "R-001");

        // Review starts as Todo — assignable
        let assignable = db.find_assignable_jobs().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, jid("R-001"));

        // Assign review — no longer assignable
        db.assign_job(&jid("R-001"), &aid("m-r")).unwrap();
        let assignable = db.find_assignable_jobs().unwrap();
        assert!(assignable.is_empty());
    }
}
