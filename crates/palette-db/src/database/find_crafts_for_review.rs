use super::*;

impl Database {
    /// Find craft jobs that a review job depends on.
    pub fn find_crafts_for_review(&self, review_id: &JobId) -> crate::Result<Vec<Job>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT t.id, t.type, t.title, t.description, t.assignee, t.status, t.priority, t.repositories, t.pr_url, t.created_at, t.updated_at, t.notes, t.assigned_at
             FROM jobs t
             JOIN dependencies d ON d.depends_on = t.id
             WHERE d.job_id = ?1 AND t.type = 'craft'",
        )?;
        let rows = stmt.query_map(params![review_id.as_ref()], |row| Ok(row_to_job(row)))?;
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
    fn find_crafts_for_review() {
        let db = test_db();
        db.create_job(&CreateJobRequest {
            id: Some(jid("C-001")),
            job_type: JobType::Craft,
            title: "Craft".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![],
        })
        .unwrap();

        db.create_job(&CreateJobRequest {
            id: Some(jid("R-001")),
            job_type: JobType::Review,
            title: "Review".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![jid("C-001")],
        })
        .unwrap();

        let crafts = db.find_crafts_for_review(&jid("R-001")).unwrap();
        assert_eq!(crafts.len(), 1);
        assert_eq!(crafts[0].id, jid("C-001"));
    }
}
