use super::super::{corrupt, corrupt_parse};
use palette_domain::job::Repository;
use serde::{Deserialize, Serialize};

/// DB-layer representation of a repository for JSON storage in SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RepositoryRow {
    pub name: String,
    pub branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_branch: Option<String>,
}

impl From<&Repository> for RepositoryRow {
    fn from(r: &Repository) -> Self {
        Self {
            name: r.name.clone(),
            branch: r.branch.clone(),
            source_branch: r.source_branch.clone(),
        }
    }
}

impl RepositoryRow {
    pub(crate) fn into_domain(self) -> Result<Repository, palette_domain::job::InvalidRepository> {
        Repository::parse(self.name, self.branch, self.source_branch)
    }
}

pub(crate) fn repository_to_json(repo: &Repository) -> String {
    let row = RepositoryRow::from(repo);
    serde_json::to_string(&row).unwrap()
}

pub(crate) fn repository_from_json(json: &str) -> crate::Result<Repository> {
    serde_json::from_str::<RepositoryRow>(json)
        .map_err(|e| corrupt(format!("repository/{e}")))?
        .into_domain()
        .map_err(corrupt_parse)
}
