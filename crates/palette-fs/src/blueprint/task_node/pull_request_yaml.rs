use palette_domain::job::{InvalidPullRequest, PullRequest};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PullRequestYaml {
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

impl PullRequestYaml {
    pub fn parse(self) -> Result<PullRequest, InvalidPullRequest> {
        PullRequest::parse(self.owner, self.repo, self.number)
    }
}
