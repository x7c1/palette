use palette_usecase::{DiffFile, GitHubReviewPort, ReviewEvent, ReviewFileComment};

pub struct MockGitHubReview;

impl GitHubReviewPort for MockGitHubReview {
    fn post_review(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
        _body: &str,
        _comments: &[ReviewFileComment],
        _event: ReviewEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn get_diff_files(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
    ) -> Result<Vec<DiffFile>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![])
    }
}
