use super::*;

impl Database {
    pub fn list_jobs(&self, filter: &JobFilter) -> crate::Result<Vec<Job>> {
        let conn = lock!(self.conn);
        let mut sql = "SELECT id, type, title, description, assignee, status, priority, repositories, pr_url, created_at, updated_at, notes, assigned_at FROM jobs WHERE 1=1".to_string();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref t) = filter.job_type {
            param_values.push(Box::new(t.as_str().to_string()));
            sql.push_str(&format!(" AND type = ?{}", param_values.len()));
        }
        if let Some(ref s) = filter.status {
            param_values.push(Box::new(s.as_str().to_string()));
            sql.push_str(&format!(" AND status = ?{}", param_values.len()));
        }
        if let Some(ref a) = filter.assignee {
            param_values.push(Box::new(a.as_ref().to_string()));
            sql.push_str(&format!(" AND assignee = ?{}", param_values.len()));
        }
        sql.push_str(" ORDER BY created_at");

        let mut stmt = conn.prepare(&sql)?;
        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_ref.as_slice(), |row| Ok(row_to_job(row)))?;

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
    fn list_jobs_with_filter() {
        let db = test_db();
        db.create_job(&CreateJobRequest {
            id: Some(jid("C-001")),
            job_type: JobType::Craft,
            title: "Craft 1".to_string(),
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
            title: "Review 1".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![],
        })
        .unwrap();

        let all = db
            .list_jobs(&JobFilter {
                job_type: None,
                status: None,
                assignee: None,
            })
            .unwrap();
        assert_eq!(all.len(), 2);

        let crafts = db
            .list_jobs(&JobFilter {
                job_type: Some(JobType::Craft),
                status: None,
                assignee: None,
            })
            .unwrap();
        assert_eq!(crafts.len(), 1);
        assert_eq!(crafts[0].id, jid("C-001"));
    }
}
