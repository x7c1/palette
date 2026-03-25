use super::super::*;

impl Database {
    pub fn get_review_comments(&self, submission_id: i64) -> crate::Result<Vec<ReviewComment>> {
        let conn = lock(&self.conn)?;
        let mut stmt = conn.prepare(
            "SELECT id, submission_id, file, line, body FROM review_comments WHERE submission_id = ?1",
        )?;
        let rows = stmt.query_map(params![submission_id], |row| {
            Ok(ReviewComment {
                id: row.get(0)?,
                submission_id: row.get(1)?,
                file: row.get(2)?,
                line: row.get(3)?,
                body: row.get(4)?,
            })
        })?;
        let mut comments = Vec::new();
        for row in rows {
            comments.push(row?);
        }
        Ok(comments)
    }
}
