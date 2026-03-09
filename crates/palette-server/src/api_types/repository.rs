use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub name: String,
    pub branch: Option<String>,
}

impl From<Repository> for domain::Repository {
    fn from(r: Repository) -> Self {
        Self {
            name: r.name,
            branch: r.branch,
        }
    }
}

impl From<domain::Repository> for Repository {
    fn from(r: domain::Repository) -> Self {
        Self {
            name: r.name,
            branch: r.branch,
        }
    }
}
