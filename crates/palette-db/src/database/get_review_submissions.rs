use super::*;

impl Database {
    pub fn get_review_submissions(
        &self,
        review_task_id: &TaskId,
    ) -> crate::Result<Vec<ReviewSubmission>> {
        let conn = lock!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, review_task_id, round, verdict, summary, created_at
             FROM review_submissions WHERE review_task_id = ?1 ORDER BY round",
        )?;
        let rows = stmt.query_map(params![review_task_id.as_ref()], |row| {
            Ok(ReviewSubmission {
                id: row.get(0)?,
                review_task_id: TaskId::new(row.get::<_, String>(1)?),
                round: row.get(2)?,
                verdict: row
                    .get::<_, String>(3)?
                    .parse()
                    .unwrap_or(Verdict::Approved),
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
