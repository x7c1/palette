use super::super::*;

impl Database {
    pub fn submit_review(
        &self,
        review_job_id: &JobId,
        req: &SubmitReviewRequest,
    ) -> crate::Result<ReviewSubmission> {
        let mut conn = lock(&self.conn)?;
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
            "INSERT INTO review_submissions (review_job_id, round, verdict_id, summary, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                review_job_id.as_ref(),
                round,
                crate::lookup::verdict_id(req.verdict),
                req.summary,
                now_str
            ],
        )?;

        let submission_id = tx.last_insert_rowid();

        for comment in &req.comments {
            tx.execute(
                "INSERT INTO review_comments (submission_id, file, line, body)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    submission_id,
                    comment.file.as_ref(),
                    comment.line.value(),
                    comment.body.as_ref()
                ],
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
    use super::super::super::test_helpers::*;

    use palette_domain::job::*;
    use palette_domain::review::*;

    #[test]
    fn submit_and_get_review() {
        let db = test_db();
        let craft_task = setup_task(&db, "wf-test:task-C-001");
        db.create_job(&CreateJobRequest::new(
            craft_task,
            Title::parse("Craft").unwrap(),
            Some(PlanPath::parse("test/C-001").unwrap()),
            None,
            None,
            JobDetail::Craft {
                repository: Repository::parse("x7c1/palette-demo", "main", None).unwrap(),
            },
        ))
        .unwrap();

        let review_task = setup_task(&db, "wf-test:task-R-001");
        let review_job = db
            .create_job(&CreateJobRequest::new(
                review_task,
                Title::parse("Review").unwrap(),
                Some(PlanPath::parse("test/R-001").unwrap()),
                None,
                None,
                JobDetail::Review {
                    perspective: None,
                    target: ReviewTarget::CraftOutput,
                },
            ))
            .unwrap();

        let sub = db
            .submit_review(
                &review_job.id,
                &SubmitReviewRequest {
                    verdict: Verdict::ChangesRequested,
                    summary: Some("Needs fixes".to_string()),
                    comments: vec![ReviewCommentInput {
                        file: FilePath::parse("src/main.rs").unwrap(),
                        line: LineNumber::parse(10).unwrap(),
                        body: CommentBody::parse("Fix this").unwrap(),
                    }],
                },
            )
            .unwrap();
        assert_eq!(sub.round, 1);
        assert_eq!(sub.verdict, Verdict::ChangesRequested);

        let sub2 = db
            .submit_review(
                &review_job.id,
                &SubmitReviewRequest {
                    verdict: Verdict::Approved,
                    summary: Some("LGTM".to_string()),
                    comments: vec![],
                },
            )
            .unwrap();
        assert_eq!(sub2.round, 2);

        let submissions = db.get_review_submissions(&review_job.id).unwrap();
        assert_eq!(submissions.len(), 2);

        let comments = db.get_review_comments(sub.id).unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].body, "Fix this");
    }
}
