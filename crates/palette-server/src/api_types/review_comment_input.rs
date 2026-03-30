use super::FieldError;
use palette_domain as domain;
use palette_domain::ReasonKey;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewCommentInput {
    pub file: String,
    pub line: i32,
    pub body: String,
}

impl ReviewCommentInput {
    /// Collect validation errors for this comment at the given index.
    pub fn collect_errors(&self, index: usize, errors: &mut Vec<FieldError>) {
        if let Err(e) = domain::review::FilePath::parse(&self.file) {
            errors.push(FieldError {
                field: format!("comments[{index}].file"),
                reason: e.reason_key(),
            });
        }

        if let Err(e) = domain::review::LineNumber::parse(self.line) {
            errors.push(FieldError {
                field: format!("comments[{index}].line"),
                reason: e.reason_key(),
            });
        }

        if let Err(e) = domain::review::CommentBody::parse(&self.body) {
            errors.push(FieldError {
                field: format!("comments[{index}].body"),
                reason: e.reason_key(),
            });
        }
    }
}
