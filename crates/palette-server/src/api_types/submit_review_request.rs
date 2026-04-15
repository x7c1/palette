use super::{InputError, Location, ReviewCommentInput, Verdict};
use palette_core::ReasonKey;
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
    pub fn validate(&self) -> Result<domain::review::SubmitReviewRequest, Vec<InputError>> {
        let mut errors = Vec::new();

        if self.comments.len() > MAX_COMMENTS {
            errors.push(InputError {
                location: Location::Body,
                hint: "comments".into(),
                reason: "comments/too_many".into(),
                help: None,
            });
        } else {
            for (i, c) in self.comments.iter().enumerate() {
                c.collect_errors(i, &mut errors);
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        // All validations passed — parse again to build domain types.
        let comments = self
            .comments
            .iter()
            .map(|c| {
                Ok(domain::review::ReviewCommentInput {
                    file: domain::review::FilePath::parse(&c.file).map_err(|e| {
                        vec![InputError {
                            location: Location::Body,
                            hint: "file".into(),
                            reason: e.reason_key(),
                            help: None,
                        }]
                    })?,
                    line: domain::review::LineNumber::parse(c.line).map_err(|e| {
                        vec![InputError {
                            location: Location::Body,
                            hint: "line".into(),
                            reason: e.reason_key(),
                            help: None,
                        }]
                    })?,
                    body: domain::review::CommentBody::parse(&c.body).map_err(|e| {
                        vec![InputError {
                            location: Location::Body,
                            hint: "body".into(),
                            reason: e.reason_key(),
                            help: None,
                        }]
                    })?,
                })
            })
            .collect::<Result<Vec<_>, Vec<InputError>>>()?;

        Ok(domain::review::SubmitReviewRequest {
            verdict: self.verdict.into(),
            summary: self.summary.clone(),
            comments,
        })
    }
}
