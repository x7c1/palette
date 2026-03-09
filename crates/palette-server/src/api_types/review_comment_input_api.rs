use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ReviewCommentInputApi {
    pub file: String,
    pub line: i32,
    pub body: String,
}
