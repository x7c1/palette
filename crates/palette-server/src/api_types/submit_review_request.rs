use super::ReviewCommentInput;
use super::Verdict;
use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SubmitReviewRequest {
    pub verdict: Verdict,
    pub summary: Option<String>,
    #[serde(default)]
    pub comments: Vec<ReviewCommentInput>,
}

// TODO: Replace From with TryFrom to validate external input (see plan 009-api-input-validation)
impl From<SubmitReviewRequest> for domain::review::SubmitReviewRequest {
    fn from(api: SubmitReviewRequest) -> Self {
        Self {
            verdict: api.verdict.into(),
            summary: api.summary,
            comments: api.comments.into_iter().map(Into::into).collect(),
        }
    }
}
