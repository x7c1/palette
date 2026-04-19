use palette_domain as domain;
use palette_domain::job::InvalidRepository;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub name: String,
    pub work_branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_branch: Option<String>,
}

impl Repository {
    /// Parse into a domain Repository, validating name and branch.
    pub fn parse(self) -> Result<domain::job::Repository, InvalidRepository> {
        domain::job::Repository::parse(self.name, self.work_branch, self.source_branch)
    }
}

impl From<domain::job::Repository> for Repository {
    fn from(r: domain::job::Repository) -> Self {
        Self {
            name: r.name,
            work_branch: r.work_branch,
            source_branch: r.source_branch,
        }
    }
}
