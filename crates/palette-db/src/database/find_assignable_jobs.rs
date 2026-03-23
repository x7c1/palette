use super::*;

impl Database {
    /// Find jobs that are assignable: status = 'todo' with no assignee.
    ///
    /// Dependencies are managed at the Task level by TaskRuleEngine,
    /// so jobs only reach 'todo' when their task dependencies are satisfied.
    ///
    /// Returns jobs ordered by priority (high > medium > low > null).
    pub fn find_assignable_jobs(&self) -> crate::Result<Vec<Job>> {
        let conn = lock!(self.conn);
        // Craft Todo = 1, Review Todo = 6
        let craft_todo = crate::lookup::craft_status_id(palette_domain::job::CraftStatus::Todo);
        let review_todo = crate::lookup::review_status_id(palette_domain::job::ReviewStatus::Todo);
        let mut stmt = conn.prepare(
            "SELECT t.id, t.task_id, t.type_id, t.title, t.plan_path, t.description, t.assignee, t.status_id, t.priority, t.repository, t.pr_url, t.created_at, t.updated_at, t.notes, t.assigned_at
             FROM jobs t
             WHERE t.status_id IN (?1, ?2) AND t.assignee IS NULL
             ORDER BY
               CASE t.priority
                 WHEN 'high' THEN 0
                 WHEN 'medium' THEN 1
                 WHEN 'low' THEN 2
                 ELSE 3
               END",
        )?;
        let rows = stmt.query_map(params![craft_todo, review_todo], row_to_job)?;
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

        // Both start as Todo — assignable immediately
        let assignable = db.find_assignable_jobs().unwrap();
        assert_eq!(assignable.len(), 2);
        assert_eq!(assignable[0].id, jid("C-001")); // high priority first
        assert_eq!(assignable[1].id, jid("C-002")); // low priority second

        // Assign one — only the other remains assignable
        db.assign_job(&jid("C-001"), &aid("m-a"), JobType::Craft)
            .unwrap();
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
        db.assign_job(&jid("R-001"), &aid("m-r"), JobType::Review)
            .unwrap();
        let assignable = db.find_assignable_jobs().unwrap();
        assert!(assignable.is_empty());
    }
}
