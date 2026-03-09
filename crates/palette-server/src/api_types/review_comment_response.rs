use palette_domain::ReviewComment;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ReviewCommentResponse {
    pub id: i64,
    pub submission_id: i64,
    pub file: String,
    pub line: i32,
    pub body: String,
}

impl From<ReviewComment> for ReviewCommentResponse {
    fn from(c: ReviewComment) -> Self {
        Self {
            id: c.id,
            submission_id: c.submission_id,
            file: c.file,
            line: c.line,
            body: c.body,
        }
    }
}
