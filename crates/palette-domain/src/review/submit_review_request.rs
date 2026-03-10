use super::{ReviewCommentInput, Verdict};

#[derive(Debug, Clone)]
pub struct SubmitReviewRequest {
    pub verdict: Verdict,
    pub summary: Option<String>,
    pub comments: Vec<ReviewCommentInput>,
}
