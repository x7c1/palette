/// Port for posting reviews to GitHub pull requests.
///
/// Implementors handle the actual HTTP calls to the GitHub API.
/// The orchestrator uses this trait after ReviewIntegrate completes
/// for standalone PR review workflows.
pub trait GitHubReviewPort: Send + Sync {
    fn post_review(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: &str,
        comments: &[ReviewFileComment],
        event: ReviewEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Return the files changed in a pull request with their diff hunk ranges.
    fn get_diff_files(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<DiffFile>, Box<dyn std::error::Error + Send + Sync>>;
}

pub struct ReviewFileComment {
    pub path: String,
    pub line: u64,
    pub body: String,
}

#[derive(Debug, Clone, Copy)]
pub enum ReviewEvent {
    Approve,
    RequestChanges,
}

pub struct DiffFile {
    pub filename: String,
    pub hunks: Vec<DiffHunk>,
}

impl DiffFile {
    pub fn contains_line(&self, line: u64) -> bool {
        self.hunks
            .iter()
            .any(|h| line >= h.start_line && line < h.start_line + h.line_count)
    }
}

pub struct DiffHunk {
    pub start_line: u64,
    pub line_count: u64,
}
