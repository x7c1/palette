use crate::review_comment_input::ReviewCommentInput;
use crate::verdict::Verdict;

#[derive(Debug, Clone)]
pub struct SubmitReviewRequest {
    pub verdict: Verdict,
    pub summary: Option<String>,
    pub comments: Vec<ReviewCommentInput>,
}
