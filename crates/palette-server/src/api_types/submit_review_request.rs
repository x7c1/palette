use super::{FieldError, ReviewCommentInput, Verdict};
use palette_domain as domain;
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

        Ok(domain::review::SubmitReviewRequest {
            verdict: self.verdict.into(),
            summary: self.summary.clone(),
            comments: self
                .comments
                .iter()
                .map(|c| domain::review::ReviewCommentInput {
                    file: c.file.clone(),
                    line: c.line,
                    body: c.body.clone(),
                })
                .collect(),
        })
    }
}
