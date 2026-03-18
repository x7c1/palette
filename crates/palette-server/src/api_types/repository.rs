use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub name: String,
    pub branch: String,
}

impl From<Repository> for domain::job::Repository {
    fn from(r: Repository) -> Self {
        Self {
            name: r.name,
            branch: r.branch,
        }
    }
}

impl From<domain::job::Repository> for Repository {
    fn from(r: domain::job::Repository) -> Self {
        Self {
            name: r.name,
            branch: r.branch,
        }
    }
}
