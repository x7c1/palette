use super::super::{corrupt, corrupt_parse};
use palette_domain::job::Repository;
use serde::{Deserialize, Serialize};

/// DB-layer representation of a repository for JSON storage in SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RepositoryRow {
    pub name: String,
    pub branch: String,
}

impl From<&Repository> for RepositoryRow {
    fn from(r: &Repository) -> Self {
        Self {
            name: r.name.clone(),
            branch: r.branch.clone(),
        }
    }
}

impl RepositoryRow {
    pub(crate) fn into_domain(self) -> Result<Repository, palette_domain::job::InvalidRepository> {
        Repository::parse(self.name, self.branch)
    }
}

pub(crate) fn repository_to_json(repo: &Repository) -> String {
    let row = RepositoryRow::from(repo);
    serde_json::to_string(&row).unwrap()
}

pub(crate) fn repository_from_json(json: &str) -> crate::Result<Repository> {
    let row: RepositoryRow = serde_json::from_str(json)
        .map_err(|e| corrupt(format!("repository/{e}")))?;
    row.into_domain().map_err(corrupt_parse)
}
