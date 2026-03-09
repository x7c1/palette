use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryApi {
    pub name: String,
    pub branch: Option<String>,
}
