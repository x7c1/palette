use super::super::*;
use super::row::{JOB_COLUMNS, into_job, read_job_row};

impl Database {
    pub fn list_jobs(&self, filter: &JobFilter) -> crate::Result<Vec<Job>> {
        let conn = lock(&self.conn)?;
        let mut sql = format!("SELECT {JOB_COLUMNS} FROM jobs WHERE 1=1");
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref t) = filter.job_type {
            param_values.push(Box::new(crate::lookup::job_type_id(*t)));
            sql.push_str(&format!(" AND type_id = ?{}", param_values.len()));
        }
        if let Some(ref s) = filter.status {
            param_values.push(Box::new(crate::lookup::job_status_id(*s)));
            sql.push_str(&format!(" AND status_id = ?{}", param_values.len()));
        }
        if let Some(ref a) = filter.assignee_id {
            param_values.push(Box::new(a.as_ref().to_string()));
            sql.push_str(&format!(" AND assignee_id = ?{}", param_values.len()));
        }
        sql.push_str(" ORDER BY created_at");

        let mut stmt = conn.prepare(&sql)?;
        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        stmt.query_map(params_ref.as_slice(), read_job_row)?
            .map(|row| into_job(row?))
            .collect::<crate::Result<Vec<_>>>()
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_helpers::*;

    use palette_domain::job::*;

    #[test]
    fn list_jobs_with_filter() {
        let db = test_db();
        let craft_task = setup_task(&db, "wf-test:task-C-001");
        let craft_job = db
            .create_job(&CreateJobRequest::new(
                craft_task,
                Title::parse("Craft 1").unwrap(),
                PlanPath::parse("test/C-001").unwrap(),
                None,
                None,
                JobDetail::Craft {
                    repository: Repository::parse("x7c1/palette-demo", "main").unwrap(),
                },
            ))
            .unwrap();

        let review_task = setup_task(&db, "wf-test:task-R-001");
        db.create_job(&CreateJobRequest::new(
            review_task,
            Title::parse("Review 1").unwrap(),
            PlanPath::parse("test/R-001").unwrap(),
            None,
            None,
            JobDetail::Review,
        ))
        .unwrap();

        let all = db
            .list_jobs(&JobFilter {
                job_type: None,
                status: None,
                assignee_id: None,
            })
            .unwrap();
        assert_eq!(all.len(), 2);

        let crafts = db
            .list_jobs(&JobFilter {
                job_type: Some(JobType::Craft),
                status: None,
                assignee_id: None,
            })
            .unwrap();
        assert_eq!(crafts.len(), 1);
        assert_eq!(crafts[0].id, craft_job.id);
    }
}
