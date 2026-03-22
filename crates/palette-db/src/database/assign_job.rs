use super::*;

impl Database {
    /// Assign a job to a member and set status to in_progress.
    pub fn assign_job(&self, job_id: &JobId, assignee: &AgentId) -> crate::Result<Job> {
        let conn = lock!(self.conn);
        let now = Utc::now().to_rfc3339();
        // Both craft and review use "in_progress" as the DB string
        let status_str = CraftStatus::InProgress.as_str(); // same string for both types
        let updated = conn.execute(
            "UPDATE jobs SET status = ?1, assignee = ?2, assigned_at = ?3, updated_at = ?4 WHERE id = ?5",
            params![status_str, assignee.as_ref(), now, now, job_id.as_ref()],
        )?;
        if updated == 0 {
            return Err(JobError::NotFound {
                job_id: job_id.clone(),
            }
            .into());
        }
        drop(conn);
        self.get_job(job_id)?.ok_or_else(|| {
            JobError::NotFound {
                job_id: job_id.clone(),
            }
            .into()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;

    use palette_domain::job::*;

    #[test]
    fn assign_job_sets_assignee_and_status() {
        let db = test_db();
        create_craft(&db, "C-001", None);

        let job = db.assign_job(&jid("C-001"), &aid("member-a")).unwrap();
        assert_eq!(job.status, JobStatus::Craft(CraftStatus::InProgress));
        assert_eq!(job.assignee, Some(aid("member-a")));
        assert!(job.assigned_at.is_some());
    }
}
