use super::FieldError;
use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewCommentInput {
    pub file: String,
    pub line: i32,
    pub body: String,
}

impl ReviewCommentInput {
    /// Collect validation hints for this comment at the given index.
    pub fn collect_hints(&self, index: usize, hints: &mut Vec<FieldError>) {
        if let Err(e) = domain::review::FilePath::parse(&self.file) {
            hints.push(FieldError {
                field: format!("comments[{index}].file"),
                reason: e.reason_key(),
            });
        }

        if let Err(e) = domain::review::LineNumber::parse(self.line) {
            hints.push(FieldError {
                field: format!("comments[{index}].line"),
                reason: e.reason_key(),
            });
        }

        if let Err(e) = domain::review::CommentBody::parse(&self.body) {
            hints.push(FieldError {
                field: format!("comments[{index}].body"),
                reason: e.reason_key(),
            });
        }
    }
}
