use super::super::*;
use palette_domain::workflow::WorkflowId;

impl Database {
    pub fn delete_review_data_by_workflow(
        &self,
        workflow_id: &WorkflowId,
    ) -> crate::Result<(usize, usize)> {
        let conn = lock(&self.conn)?;
        let deleted_comments = conn.execute(
            "DELETE FROM review_comments
             WHERE submission_id IN (
               SELECT rs.id
               FROM review_submissions rs
               JOIN jobs j ON j.id = rs.review_job_id
               JOIN tasks t ON t.id = j.task_id
               WHERE t.workflow_id = ?1
             )",
            [workflow_id.as_ref()],
        )?;
        let deleted_submissions = conn.execute(
            "DELETE FROM review_submissions
             WHERE review_job_id IN (
               SELECT j.id
               FROM jobs j
               JOIN tasks t ON t.id = j.task_id
               WHERE t.workflow_id = ?1
             )",
            [workflow_id.as_ref()],
        )?;
        Ok((deleted_comments, deleted_submissions))
    }
}
