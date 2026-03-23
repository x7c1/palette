use palette_domain::job::Repository;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct RepositoryYaml {
    pub name: String,
    pub branch: String,
}

impl From<RepositoryYaml> for Repository {
    fn from(r: RepositoryYaml) -> Self {
        Repository {
            name: r.name,
            branch: r.branch,
        }
    }
}
