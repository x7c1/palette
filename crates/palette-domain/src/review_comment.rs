#[derive(Debug, Clone)]
pub struct ReviewComment {
    pub id: i64,
    pub submission_id: i64,
    pub file: String,
    pub line: i32,
    pub body: String,
}
