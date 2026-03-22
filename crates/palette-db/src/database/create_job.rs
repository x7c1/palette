use super::*;

impl Database {
    pub fn create_job(&self, req: &CreateJobRequest) -> crate::Result<Job> {
        let mut conn = lock!(self.conn);
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let id = req
            .id
            .clone()
            .unwrap_or_else(|| JobId::generate(req.job_type));

        let repos_json = req
            .repository
            .as_ref()
            .map(repository_row::repository_to_json);

        let initial_status = JobStatus::todo(req.job_type);

        let tx = conn.transaction()?;

        tx.execute(
            "INSERT INTO jobs (id, task_id, type, title, plan_path, description, assignee, status, priority, repository, pr_url, created_at, updated_at, notes, assigned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, ?11, ?12, NULL, NULL)",
            params![
                id.as_ref(),
                req.task_id.as_ref(),
                req.job_type.as_str(),
                req.title,
                req.plan_path,
                req.description,
                req.assignee.as_ref().map(|a| a.as_ref()),
                initial_status.as_str(),
                req.priority.map(|p| p.as_str()),
                repos_json,
                now_str,
                now_str,
            ],
        )?;

        let job =
            query_job(&tx, &id)?.ok_or_else(|| Error::Job(JobError::NotFound { job_id: id }))?;

        tx.commit()?;
        Ok(job)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;

    use palette_domain::job::*;

    #[test]
    fn create_and_get_job() {
        let db = test_db();
        let task_id = setup_task(&db, "task-C-001");
        let job = db
            .create_job(&CreateJobRequest {
                task_id,
                id: Some(jid("C-001")),
                job_type: JobType::Craft,
                title: "Implement feature".to_string(),
                plan_path: "2026/feature-x/api-impl".to_string(),
                description: Some("Details".to_string()),
                assignee: Some(aid("member-a")),
                priority: Some(Priority::High),
                repository: Some(Repository {
                    name: "x7c1/palette".to_string(),
                    branch: "feature/test".to_string(),
                }),
            })
            .unwrap();

        assert_eq!(job.id, jid("C-001"));
        assert_eq!(job.job_type, JobType::Craft);
        assert_eq!(job.status, JobStatus::Craft(CraftStatus::Todo));
        assert_eq!(job.priority, Some(Priority::High));

        let fetched = db.get_job(&jid("C-001")).unwrap().unwrap();
        assert_eq!(fetched.title, "Implement feature");
    }
}
