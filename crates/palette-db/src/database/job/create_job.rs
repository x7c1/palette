use super::super::*;
use super::row::query_job;

impl Database {
    pub fn create_job(&self, req: &CreateJobRequest) -> crate::Result<Job> {
        let mut conn = lock(&self.conn)?;
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let job_type = req.detail.job_type();
        let id = JobId::generate(job_type);

        let repos_json = req
            .detail
            .repository()
            .map(super::repository_row::repository_to_json);

        let command = req.detail.command();
        let perspective = req.detail.perspective();

        let initial_status = JobStatus::todo(job_type);

        let tx = conn.transaction()?;

        tx.execute(
            "INSERT INTO jobs (id, task_id, type_id, title, plan_path, assignee_id, status_id, priority_id, repository, command, perspective, created_at, updated_at, notes, assigned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, NULL, NULL)",
            params![
                id.as_ref(),
                req.task_id.as_ref(),
                crate::lookup::job_type_id(job_type),
                req.title.as_ref(),
                req.plan_path.as_ref(),
                req.assignee_id.as_ref().map(|a| a.as_ref()),
                crate::lookup::job_status_id(initial_status),
                req.priority.map(crate::lookup::priority_id),
                repos_json,
                command,
                perspective,
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
    use super::super::super::test_helpers::*;

    use palette_domain::job::*;

    #[test]
    fn create_and_get_job() {
        let db = test_db();
        setup_worker(&db, "member-a");
        let task_id = setup_task(&db, "wf-test:task-C-001");
        let job = db
            .create_job(&CreateJobRequest::new(
                task_id,
                Title::parse("Implement feature").unwrap(),
                PlanPath::parse("2026/feature-x/api-impl").unwrap(),
                Some(wid("member-a")),
                Some(Priority::High),
                JobDetail::Craft {
                    repository: Repository::parse("x7c1/palette-demo", "feature/test").unwrap(),
                },
            ))
            .unwrap();

        assert_eq!(job.detail.job_type(), JobType::Craft);
        assert_eq!(job.status, JobStatus::Craft(CraftStatus::Todo));
        assert_eq!(job.priority, Some(Priority::High));

        let fetched = db.get_job(&job.id).unwrap().unwrap();
        assert_eq!(fetched.title.as_ref(), "Implement feature");
    }
}
