use super::ReviewCommentInput;
use super::Verdict;
use palette_domain as domain;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SubmitReviewRequest {
    pub verdict: Verdict,
    pub summary: Option<String>,
    #[serde(default)]
    pub comments: Vec<ReviewCommentInput>,
}

impl From<SubmitReviewRequest> for domain::SubmitReviewRequest {
    fn from(api: SubmitReviewRequest) -> Self {
        Self {
            verdict: api.verdict.into(),
            summary: api.summary,
            comments: api
                .comments
                .into_iter()
                .map(|c| domain::ReviewCommentInput {
                    file: c.file,
                    line: c.line,
                    body: c.body,
                })
                .collect(),
        }
    }
}
