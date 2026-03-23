use super::*;

impl Database {
    pub fn get_review_submissions(
        &self,
        review_job_id: &JobId,
    ) -> crate::Result<Vec<ReviewSubmission>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, review_job_id, round, verdict_id, summary, created_at
             FROM review_submissions WHERE review_job_id = ?1 ORDER BY round",
        )?;
        let rows = stmt.query_map(params![review_job_id.as_ref()], |row| {
            let verdict_id: i64 = row.get(3)?;
            let verdict =
                crate::lookup::verdict_from_id(verdict_id).map_err(super::id_conversion_error)?;
            Ok(ReviewSubmission {
                id: row.get(0)?,
                review_job_id: JobId::new(row.get::<_, String>(1)?),
                round: row.get(2)?,
                verdict,
                summary: row.get(4)?,
                created_at: parse_datetime(&row.get::<_, String>(5)?),
            })
        })?;
        let mut submissions = Vec::new();
        for row in rows {
            submissions.push(row?);
        }
        Ok(submissions)
    }
}
