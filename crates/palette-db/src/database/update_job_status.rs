use super::*;

impl Database {
    pub fn update_job_status(&self, id: &JobId, status: JobStatus) -> crate::Result<Job> {
        let conn = lock!(self.conn);
        let now = Utc::now().to_rfc3339();
        let updated = conn.execute(
            "UPDATE jobs SET status_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![crate::lookup::job_status_id(status), now, id.as_ref()],
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
        let task_id = setup_task(&db, "task-C-001");
        db.create_job(&CreateJobRequest {
            task_id,
            id: Some(jid("C-001")),
            job_type: JobType::Craft,
            title: "Craft".to_string(),
            plan_path: "test/C-001".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repository: None,
        })
        .unwrap();

        let updated = db
            .update_job_status(&jid("C-001"), JobStatus::Craft(CraftStatus::InProgress))
            .unwrap();
        assert_eq!(updated.status, JobStatus::Craft(CraftStatus::InProgress));
    }
}
