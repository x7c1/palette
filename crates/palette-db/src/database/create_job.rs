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

        // Craft jobs start as Draft; review jobs start as Todo
        let initial_status = match req.job_type {
            JobType::Craft => JobStatus::Draft,
            JobType::Review => JobStatus::Todo,
        };

        let tx = conn.transaction()?;

        tx.execute(
            "INSERT INTO jobs (id, type, title, description, assignee, status, priority, repositories, pr_url, created_at, updated_at, notes, assigned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?10, NULL, NULL)",
            params![
                id.as_ref(),
                req.job_type.as_str(),
                req.title,
                req.description,
                req.assignee.as_ref().map(|a| a.as_ref()),
                initial_status.as_str(),
                req.priority.map(|p| p.as_str()),
                repos_json,
                now_str,
                now_str,
            ],
        )?;

        for dep in &req.depends_on {
            tx.execute(
                "INSERT INTO dependencies (job_id, depends_on) VALUES (?1, ?2)",
                params![id.as_ref(), dep.as_ref()],
            )?;
        }

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
        let job = db
            .create_job(&CreateJobRequest {
                id: Some(jid("C-001")),
                job_type: JobType::Craft,
                title: "Implement feature".to_string(),
                description: Some("Details".to_string()),
                assignee: Some(aid("member-a")),
                priority: Some(Priority::High),
                repository: Some(Repository {
                    name: "x7c1/palette".to_string(),
                    branch: Some("feature/test".to_string()),
                }),
                depends_on: vec![],
            })
            .unwrap();

        assert_eq!(job.id, jid("C-001"));
        assert_eq!(job.job_type, JobType::Craft);
        assert_eq!(job.status, JobStatus::Draft);
        assert_eq!(job.priority, Some(Priority::High));

        let fetched = db.get_job(&jid("C-001")).unwrap().unwrap();
        assert_eq!(fetched.title, "Implement feature");
    }

    #[test]
    fn create_job_with_dependencies() {
        let db = test_db();
        db.create_job(&CreateJobRequest {
            id: Some(jid("C-001")),
            job_type: JobType::Craft,
            title: "Craft job".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repository: None,
            depends_on: vec![],
        })
        .unwrap();

        db.create_job(&CreateJobRequest {
            id: Some(jid("R-001")),
            job_type: JobType::Review,
            title: "Review job".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repository: None,
            depends_on: vec![jid("C-001")],
        })
        .unwrap();

        let deps = db.get_dependencies(&jid("R-001")).unwrap();
        assert_eq!(deps, vec![jid("C-001")]);

        let dependents = db.get_dependents(&jid("C-001")).unwrap();
        assert_eq!(dependents, vec![jid("R-001")]);
    }
}
