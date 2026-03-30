/// Raw database representation of a review submission record.
pub(crate) struct ReviewSubmissionRow {
    pub id: i64,
    pub review_job_id: String,
    pub round: i64,
    pub verdict_id: i64,
    pub summary: Option<String>,
    pub created_at: String,
}
