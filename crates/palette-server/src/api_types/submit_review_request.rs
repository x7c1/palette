use super::{FieldError, ReviewCommentInput, Verdict};
use palette_domain as domain;
use palette_domain::ReasonKey;
use serde::{Deserialize, Serialize};

const MAX_COMMENTS: usize = 200;

#[derive(Debug, Serialize, Deserialize)]
pub struct SubmitReviewRequest {
    pub verdict: Verdict,
    pub summary: Option<String>,
    #[serde(default)]
    pub comments: Vec<ReviewCommentInput>,
}

impl SubmitReviewRequest {
    pub fn validate(&self) -> Result<domain::review::SubmitReviewRequest, Vec<FieldError>> {
        let mut hints = Vec::new();

        if self.comments.len() > MAX_COMMENTS {
            hints.push(FieldError {
                field: "comments".into(),
                reason: "comments/too_many".into(),
            });
        } else {
            for (i, c) in self.comments.iter().enumerate() {
                c.collect_hints(i, &mut hints);
            }
        }

        if !hints.is_empty() {
            return Err(hints);
        }

        // All validations passed — parse again to build domain types.
        let comments = self
            .comments
            .iter()
            .map(|c| {
                Ok(domain::review::ReviewCommentInput {
                    file: domain::review::FilePath::parse(&c.file).map_err(|e| {
                        vec![FieldError {
                            field: "file".into(),
                            reason: e.reason_key(),
                        }]
                    })?,
                    line: domain::review::LineNumber::parse(c.line).map_err(|e| {
                        vec![FieldError {
                            field: "line".into(),
                            reason: e.reason_key(),
                        }]
                    })?,
                    body: domain::review::CommentBody::parse(&c.body).map_err(|e| {
                        vec![FieldError {
                            field: "body".into(),
                            reason: e.reason_key(),
                        }]
                    })?,
                })
            })
            .collect::<Result<Vec<_>, Vec<FieldError>>>()?;

        Ok(domain::review::SubmitReviewRequest {
            verdict: self.verdict.into(),
            summary: self.summary.clone(),
            comments,
        })
    }
}
