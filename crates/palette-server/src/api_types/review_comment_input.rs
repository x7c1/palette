use super::FieldHint;
use serde::{Deserialize, Serialize};

const MAX_FILE_PATH_LEN: usize = 1024;
const MAX_LINE: i32 = 1_000_000;
const MAX_BODY_LEN: usize = 10_000;

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewCommentInput {
    pub file: String,
    pub line: i32,
    pub body: String,
}

impl ReviewCommentInput {
    /// Collect validation hints for this comment at the given index.
    pub fn collect_hints(&self, index: usize, hints: &mut Vec<FieldHint>) {
        if self.file.trim().is_empty() {
            hints.push(FieldHint {
                field: format!("comments[{index}].file"),
                reason: "required".into(),
            });
        } else if self.file.len() > MAX_FILE_PATH_LEN {
            hints.push(FieldHint {
                field: format!("comments[{index}].file"),
                reason: "too_long".into(),
            });
        }

        if self.line < 0 {
            hints.push(FieldHint {
                field: format!("comments[{index}].line"),
                reason: "negative".into(),
            });
        } else if self.line > MAX_LINE {
            hints.push(FieldHint {
                field: format!("comments[{index}].line"),
                reason: "too_large".into(),
            });
        }

        if self.body.trim().is_empty() {
            hints.push(FieldHint {
                field: format!("comments[{index}].body"),
                reason: "required".into(),
            });
        } else if self.body.len() > MAX_BODY_LEN {
            hints.push(FieldHint {
                field: format!("comments[{index}].body"),
                reason: "too_long".into(),
            });
        }
    }
}
