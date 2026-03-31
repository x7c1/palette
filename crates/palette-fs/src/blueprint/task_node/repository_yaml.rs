use palette_domain::job::{InvalidRepository, Repository};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct RepositoryYaml {
    pub name: String,
    pub branch: String,
}

impl RepositoryYaml {
    /// Parse into a domain Repository, validating name and branch.
    pub fn parse(self) -> Result<Repository, InvalidRepository> {
        Repository::parse(self.name, self.branch)
    }
}
