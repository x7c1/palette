use palette_domain::job::{InvalidRepository, Repository};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct RepositoryYaml {
    pub name: String,
    pub work_branch: String,
    #[serde(default)]
    pub source_branch: Option<String>,
}

impl RepositoryYaml {
    /// Parse into a domain Repository, validating name and branch.
    pub fn parse(self) -> Result<Repository, InvalidRepository> {
        Repository::parse(self.name, self.work_branch, self.source_branch)
    }
}
