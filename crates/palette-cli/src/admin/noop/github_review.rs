use super::{BoxErr, unsupported};
use palette_usecase::{GitHubReviewPort, ReviewEvent, ReviewFileComment};

pub(in crate::admin) struct NoopGitHubReview;

impl GitHubReviewPort for NoopGitHubReview {
    fn post_review(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
        _body: &str,
        _comments: &[ReviewFileComment],
        _event: ReviewEvent,
    ) -> Result<(), BoxErr> {
        unsupported("post_review")
    }

    fn get_diff_files(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
    ) -> Result<Vec<String>, BoxErr> {
        unsupported("get_diff_files")
    }
}
