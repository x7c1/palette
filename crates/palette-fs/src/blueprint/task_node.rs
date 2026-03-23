use palette_domain::job::{JobType, Priority, Repository};
use serde::Deserialize;

/// A node in the Blueprint's Task tree.
#[derive(Debug, Deserialize)]
pub struct TaskNode {
    pub key: String,
    pub plan_path: Option<String>,
    #[serde(rename = "type")]
    pub job_type: Option<JobTypeYaml>,
    pub priority: Option<PriorityYaml>,
    pub repository: Option<RepositoryYaml>,
    #[serde(default)]
    pub children: Vec<TaskNode>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobTypeYaml {
    Craft,
    Review,
}

impl From<JobTypeYaml> for JobType {
    fn from(t: JobTypeYaml) -> Self {
        match t {
            JobTypeYaml::Craft => JobType::Craft,
            JobTypeYaml::Review => JobType::Review,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorityYaml {
    High,
    Medium,
    Low,
}

impl From<PriorityYaml> for Priority {
    fn from(p: PriorityYaml) -> Self {
        match p {
            PriorityYaml::High => Priority::High,
            PriorityYaml::Medium => Priority::Medium,
            PriorityYaml::Low => Priority::Low,
        }
    }
}

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
