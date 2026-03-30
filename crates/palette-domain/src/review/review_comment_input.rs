use super::{CommentBody, FilePath, LineNumber};

#[derive(Debug, Clone)]
pub struct ReviewCommentInput {
    pub file: FilePath,
    pub line: LineNumber,
    pub body: CommentBody,
}
