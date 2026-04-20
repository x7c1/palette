use palette_usecase::{
    DiffFile, GitHubReviewPort, PullRequestRefs, ReviewEvent, ReviewFileComment,
};

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

    fn get_pr_base(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
    ) -> Result<PullRequestRefs, Box<dyn std::error::Error + Send + Sync>> {
        Ok(PullRequestRefs {
            base_ref: "main".to_string(),
            base_sha: "0000000000000000000000000000000000000000".to_string(),
            head_ref: "feature".to_string(),
            head_sha: "1111111111111111111111111111111111111111".to_string(),
        })
    }
}
