use serde::Deserialize;

/// A repository with name (org/repo) and branch.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct RepositoryEntry {
    pub name: String,
    pub branch: String,
}
