const MAX_OWNER_LEN: usize = 256;
const MAX_REPO_LEN: usize = 256;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct PullRequest {
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

impl PullRequest {
    pub fn parse(
        owner: impl Into<String>,
        repo: impl Into<String>,
        number: u64,
    ) -> Result<Self, InvalidPullRequest> {
        let owner = owner.into();
        let repo = repo.into();
        if owner.is_empty() {
            return Err(InvalidPullRequest::OwnerEmpty);
        }
        if owner.len() > MAX_OWNER_LEN {
            return Err(InvalidPullRequest::OwnerTooLong { len: owner.len() });
        }
        if repo.is_empty() {
            return Err(InvalidPullRequest::RepoEmpty);
        }
        if repo.len() > MAX_REPO_LEN {
            return Err(InvalidPullRequest::RepoTooLong { len: repo.len() });
        }
        if number == 0 {
            return Err(InvalidPullRequest::NumberZero);
        }
        Ok(Self {
            owner,
            repo,
            number,
        })
    }

    pub fn full_name(&self) -> String {
        format!("{}/{}#{}", self.owner, self.repo, self.number)
    }
}

impl std::fmt::Display for PullRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}#{}", self.owner, self.repo, self.number)
    }
}

#[derive(Debug, palette_macros::ReasonKey)]
pub enum InvalidPullRequest {
    OwnerEmpty,
    OwnerTooLong { len: usize },
    RepoEmpty,
    RepoTooLong { len: usize },
    NumberZero,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_pull_request() {
        let pr = PullRequest::parse("x7c1", "palette", 42).unwrap();
        assert_eq!(pr.owner, "x7c1");
        assert_eq!(pr.repo, "palette");
        assert_eq!(pr.number, 42);
        assert_eq!(pr.full_name(), "x7c1/palette#42");
    }

    #[test]
    fn rejects_empty_owner() {
        let err = PullRequest::parse("", "palette", 1).unwrap_err();
        assert!(matches!(err, InvalidPullRequest::OwnerEmpty));
    }

    #[test]
    fn rejects_empty_repo() {
        let err = PullRequest::parse("x7c1", "", 1).unwrap_err();
        assert!(matches!(err, InvalidPullRequest::RepoEmpty));
    }

    #[test]
    fn rejects_zero_number() {
        let err = PullRequest::parse("x7c1", "palette", 0).unwrap_err();
        assert!(matches!(err, InvalidPullRequest::NumberZero));
    }
}
