use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewCommentResponse {
    pub id: i64,
    pub submission_id: i64,
    pub file: String,
    pub line: i32,
    pub body: String,
}

impl From<domain::ReviewComment> for ReviewCommentResponse {
    fn from(c: domain::ReviewComment) -> Self {
        Self {
            id: c.id,
            submission_id: c.submission_id,
            file: c.file,
            line: c.line,
            body: c.body,
        }
    }
}
