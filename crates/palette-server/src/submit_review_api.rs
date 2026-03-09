use crate::review_comment_input_api::ReviewCommentInputApi;
use crate::verdict_api::VerdictApi;
use palette_domain::{ReviewCommentInput, SubmitReviewRequest};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SubmitReviewApi {
    pub verdict: VerdictApi,
    pub summary: Option<String>,
    #[serde(default)]
    pub comments: Vec<ReviewCommentInputApi>,
}

impl From<SubmitReviewApi> for SubmitReviewRequest {
    fn from(api: SubmitReviewApi) -> Self {
        Self {
            verdict: api.verdict.into(),
            summary: api.summary,
            comments: api
                .comments
                .into_iter()
                .map(|c| ReviewCommentInput {
                    file: c.file,
                    line: c.line,
                    body: c.body,
                })
                .collect(),
        }
    }
}
