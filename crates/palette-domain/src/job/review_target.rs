use super::PullRequest;

/// Distinguishes the source being reviewed.
///
/// - `CraftOutput`: A crafter's deliverable (traditional review flow).
/// - `PullRequest`: An existing GitHub PR (standalone review flow).
#[derive(Debug, Clone)]
pub enum ReviewTarget {
    CraftOutput,
    PullRequest(PullRequest),
}

impl ReviewTarget {
    pub fn pull_request(&self) -> Option<&PullRequest> {
        match self {
            ReviewTarget::PullRequest(pr) => Some(pr),
            ReviewTarget::CraftOutput => None,
        }
    }

    pub fn is_pull_request(&self) -> bool {
        matches!(self, ReviewTarget::PullRequest(_))
    }
}
