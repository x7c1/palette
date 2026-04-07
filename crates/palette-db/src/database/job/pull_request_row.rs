use super::super::{corrupt, corrupt_parse};
use palette_domain::job::PullRequest;
use serde::{Deserialize, Serialize};

/// DB-layer representation of a pull request for JSON storage in SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PullRequestRow {
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

impl From<&PullRequest> for PullRequestRow {
    fn from(pr: &PullRequest) -> Self {
        Self {
            owner: pr.owner.clone(),
            repo: pr.repo.clone(),
            number: pr.number,
        }
    }
}

impl PullRequestRow {
    pub(crate) fn into_domain(
        self,
    ) -> Result<PullRequest, palette_domain::job::InvalidPullRequest> {
        PullRequest::parse(self.owner, self.repo, self.number)
    }
}

pub(crate) fn pull_request_to_json(pr: &PullRequest) -> String {
    let row = PullRequestRow::from(pr);
    serde_json::to_string(&row).unwrap()
}

pub(crate) fn pull_request_from_json(json: &str) -> crate::Result<PullRequest> {
    serde_json::from_str::<PullRequestRow>(json)
        .map_err(|e| corrupt(format!("pull_request/{e}")))?
        .into_domain()
        .map_err(corrupt_parse)
}
