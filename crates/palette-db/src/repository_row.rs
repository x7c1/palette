use palette_domain::Repository;
use serde::{Deserialize, Serialize};

/// DB-layer representation of a repository for JSON storage in SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RepositoryRow {
    pub name: String,
    pub branch: Option<String>,
}

impl From<&Repository> for RepositoryRow {
    fn from(r: &Repository) -> Self {
        Self {
            name: r.name.clone(),
            branch: r.branch.clone(),
        }
    }
}

impl From<RepositoryRow> for Repository {
    fn from(r: RepositoryRow) -> Self {
        Self {
            name: r.name,
            branch: r.branch,
        }
    }
}

pub(crate) fn repositories_to_json(repos: &[Repository]) -> String {
    let rows: Vec<RepositoryRow> = repos.iter().map(RepositoryRow::from).collect();
    serde_json::to_string(&rows).unwrap()
}

pub(crate) fn repositories_from_json(json: &str) -> Option<Vec<Repository>> {
    let rows: Vec<RepositoryRow> = serde_json::from_str(json).ok()?;
    Some(rows.into_iter().map(Repository::from).collect())
}
