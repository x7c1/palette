use super::*;

impl Database {
    /// Find review jobs that depend on the given craft job.
    pub fn find_reviews_for_craft(&self, craft_id: &JobId) -> crate::Result<Vec<Job>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT t.id, t.type, t.title, t.description, t.assignee, t.status, t.priority, t.repositories, t.pr_url, t.created_at, t.updated_at, t.notes, t.assigned_at
             FROM jobs t
             JOIN dependencies d ON d.job_id = t.id
             WHERE d.depends_on = ?1 AND t.type = 'review'",
        )?;
        let rows = stmt.query_map(params![craft_id.as_ref()], |row| Ok(row_to_job(row)))?;
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
    fn find_reviews_for_craft() {
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

        let reviews = db.find_reviews_for_craft(&jid("C-001")).unwrap();
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].id, jid("R-001"));
    }
}
