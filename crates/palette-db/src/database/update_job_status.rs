use super::*;

impl Database {
    pub fn update_job_status(&self, id: &JobId, status: JobStatus) -> crate::Result<Job> {
        let conn = lock!(self.conn);
        let now = Utc::now().to_rfc3339();
        let updated = conn.execute(
            "UPDATE jobs SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status.as_str(), now, id.as_ref()],
        )?;
        if updated == 0 {
            return Err(JobError::NotFound { job_id: id.clone() }.into());
        }
        drop(conn);
        self.get_job(id)?
            .ok_or_else(|| JobError::NotFound { job_id: id.clone() }.into())
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;

    use palette_domain::job::*;

    #[test]
    fn update_job_status() {
        let db = test_db();
        db.create_job(&CreateJobRequest {
            id: Some(jid("C-001")),
            job_type: JobType::Craft,
            title: "Craft".to_string(),
            plan_path: "test/C-001".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repository: None,
            depends_on: vec![],
        })
        .unwrap();

        let updated = db
            .update_job_status(&jid("C-001"), JobStatus::InProgress)
            .unwrap();
        assert_eq!(updated.status, JobStatus::InProgress);
    }
}
