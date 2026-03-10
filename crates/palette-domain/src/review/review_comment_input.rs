#[derive(Debug, Clone)]
pub struct ReviewCommentInput {
    pub file: String,
    pub line: i32,
    pub body: String,
}
