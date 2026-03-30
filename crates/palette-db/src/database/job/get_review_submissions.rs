use super::super::*;
use crate::models::ReviewSubmissionRow;

fn read_review_submission_row(row: &rusqlite::Row) -> rusqlite::Result<ReviewSubmissionRow> {
    Ok(ReviewSubmissionRow {
        id: row.get("id")?,
        review_job_id: row.get("review_job_id")?,
        round: row.get("round")?,
        verdict_id: row.get("verdict_id")?,
        summary: row.get("summary")?,
        created_at: row.get("created_at")?,
    })
}

fn into_review_submission(row: ReviewSubmissionRow) -> crate::Result<ReviewSubmission> {
    let verdict = crate::lookup::verdict_from_id(row.verdict_id)
        .map_err(|e| crate::Error::Internal(Box::new(e)))?;

    Ok(ReviewSubmission {
        id: row.id,
        review_job_id: JobId::new(row.review_job_id),
        round: row.round as i32,
        verdict,
        summary: row.summary,
        created_at: parse_datetime(&row.created_at),
    })
}

impl Database {
    pub fn get_review_submissions(
        &self,
        review_job_id: &JobId,
    ) -> crate::Result<Vec<ReviewSubmission>> {
        let conn = lock(&self.conn)?;
        let mut stmt = conn.prepare(
            "SELECT id, review_job_id, round, verdict_id, summary, created_at
             FROM review_submissions WHERE review_job_id = ?1 ORDER BY round",
        )?;
        let rows = stmt.query_map(params![review_job_id.as_ref()], read_review_submission_row)?;
        let mut submissions = Vec::new();
        for row in rows {
            submissions.push(into_review_submission(row?)?);
        }
        Ok(submissions)
    }
}
