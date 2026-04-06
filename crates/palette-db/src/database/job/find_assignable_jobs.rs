use super::super::*;
use super::row::{JOB_COLUMNS, into_job, read_job_row};

impl Database {
    /// Find jobs that are assignable: status = 'todo' with no assignee_id.
    ///
    /// Dependencies are managed at the Task level by TaskRuleEngine,
    /// so jobs only reach 'todo' when their task dependencies are satisfied.
    ///
    /// Returns jobs ordered by priority (high > medium > low > null).
    pub fn find_assignable_jobs(&self) -> crate::Result<Vec<Job>> {
        let conn = lock(&self.conn)?;
        // Todo status IDs for each job type
        let craft_todo = crate::lookup::craft_status_id(palette_domain::job::CraftStatus::Todo);
        let review_todo = crate::lookup::review_status_id(palette_domain::job::ReviewStatus::Todo);
        let orchestrator_todo =
            crate::lookup::job_status_id(palette_domain::job::JobStatus::Orchestrator(
                palette_domain::job::MechanizedStatus::Todo,
            ));
        let operator_todo = crate::lookup::job_status_id(palette_domain::job::JobStatus::Operator(
            palette_domain::job::MechanizedStatus::Todo,
        ));
        let mut stmt = conn.prepare(&format!(
            "SELECT {JOB_COLUMNS}
             FROM jobs t
             WHERE t.status_id IN (?1, ?2, ?3, ?4) AND t.assignee_id IS NULL
             ORDER BY
               CASE t.priority_id
                 WHEN 1 THEN 0
                 WHEN 2 THEN 1
                 WHEN 3 THEN 2
                 ELSE 3
               END"
        ))?;
        stmt.query_map(
            params![craft_todo, review_todo, orchestrator_todo, operator_todo],
            read_job_row,
        )?
        .map(|row| into_job(row?))
        .collect::<crate::Result<Vec<_>>>()
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_helpers::*;

    use palette_domain::job::*;

    #[test]
    fn find_assignable_craft_jobs() {
        let db = test_db();
        setup_worker(&db, "m-a");
        let craft1 = create_craft(&db, "C-001", Some(Priority::High));
        let craft2 = create_craft(&db, "C-002", Some(Priority::Low));

        // Both start as Todo — assignable immediately
        let assignable = db.find_assignable_jobs().unwrap();
        assert_eq!(assignable.len(), 2);
        assert_eq!(assignable[0].id, craft1.id); // high priority first
        assert_eq!(assignable[1].id, craft2.id); // low priority second

        // Assign one — only the other remains assignable
        db.assign_job(&craft1.id, &wid("m-a"), JobType::Craft)
            .unwrap();
        let assignable = db.find_assignable_jobs().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, craft2.id);
    }

    #[test]
    fn find_assignable_review_jobs() {
        let db = test_db();
        setup_worker(&db, "m-r");
        let review = create_review(&db, "R-001");

        // Review starts as Todo — assignable
        let assignable = db.find_assignable_jobs().unwrap();
        assert_eq!(assignable.len(), 1);
        assert_eq!(assignable[0].id, review.id);

        // Assign review — no longer assignable
        db.assign_job(&review.id, &wid("m-r"), JobType::Review)
            .unwrap();
        let assignable = db.find_assignable_jobs().unwrap();
        assert!(assignable.is_empty());
    }
}
