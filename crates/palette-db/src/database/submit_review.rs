use super::*;

impl Database {
    pub fn submit_review(
        &self,
        review_job_id: &JobId,
        req: &SubmitReviewRequest,
    ) -> crate::Result<ReviewSubmission> {
        let mut conn = lock!(self.conn);
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        let tx = conn.transaction()?;

        // Determine round number
        let round: i32 = tx
            .query_row(
                "SELECT COALESCE(MAX(round), 0) FROM review_submissions WHERE review_job_id = ?1",
                params![review_job_id.as_ref()],
                |row| row.get(0),
            )
            .unwrap_or(0)
            + 1;

        tx.execute(
            "INSERT INTO review_submissions (review_job_id, round, verdict, summary, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                review_job_id.as_ref(),
                round,
                req.verdict.as_str(),
                req.summary,
                now_str
            ],
        )?;

        let submission_id = tx.last_insert_rowid();

        for comment in &req.comments {
            tx.execute(
                "INSERT INTO review_comments (submission_id, file, line, body)
                 VALUES (?1, ?2, ?3, ?4)",
                params![submission_id, comment.file, comment.line, comment.body],
            )?;
        }

        tx.commit()?;

        Ok(ReviewSubmission {
            id: submission_id,
            review_job_id: review_job_id.clone(),
            round,
            verdict: req.verdict,
            summary: req.summary.clone(),
            created_at: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;

    use palette_domain::job::*;
    use palette_domain::review::*;

    #[test]
    fn submit_and_get_review() {
        let db = test_db();
        db.create_job(&CreateJobRequest {
            id: Some(jid("C-001")),
            job_type: JobType::Craft,
            title: "Craft".to_string(),
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
            title: "Review".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![jid("C-001")],
        })
        .unwrap();

        let sub = db
            .submit_review(
                &jid("R-001"),
                &SubmitReviewRequest {
                    verdict: Verdict::ChangesRequested,
                    summary: Some("Needs fixes".to_string()),
                    comments: vec![ReviewCommentInput {
                        file: "src/main.rs".to_string(),
                        line: 10,
                        body: "Fix this".to_string(),
                    }],
                },
            )
            .unwrap();
        assert_eq!(sub.round, 1);
        assert_eq!(sub.verdict, Verdict::ChangesRequested);

        let sub2 = db
            .submit_review(
                &jid("R-001"),
                &SubmitReviewRequest {
                    verdict: Verdict::Approved,
                    summary: Some("LGTM".to_string()),
                    comments: vec![],
                },
            )
            .unwrap();
        assert_eq!(sub2.round, 2);

        let submissions = db.get_review_submissions(&jid("R-001")).unwrap();
        assert_eq!(submissions.len(), 2);

        let comments = db.get_review_comments(sub.id).unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].body, "Fix this");
    }
}
