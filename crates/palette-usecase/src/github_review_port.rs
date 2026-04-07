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

    /// Return the list of files changed in a pull request.
    fn get_diff_files(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>>;
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
