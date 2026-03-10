use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewCommentInput {
    pub file: String,
    pub line: i32,
    pub body: String,
}

// TODO: Replace From with TryFrom to validate external input (see plan 009-api-input-validation)
impl From<ReviewCommentInput> for domain::review::ReviewCommentInput {
    fn from(c: ReviewCommentInput) -> Self {
        Self {
            file: c.file,
            line: c.line,
            body: c.body,
        }
    }
}
