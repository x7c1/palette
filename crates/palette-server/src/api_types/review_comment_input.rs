use palette_domain as domain;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ReviewCommentInput {
    pub file: String,
    pub line: i32,
    pub body: String,
}

impl From<ReviewCommentInput> for domain::ReviewCommentInput {
    fn from(c: ReviewCommentInput) -> Self {
        Self {
            file: c.file,
            line: c.line,
            body: c.body,
        }
    }
}
