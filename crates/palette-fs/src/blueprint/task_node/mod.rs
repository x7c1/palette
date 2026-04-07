mod job_type_yaml;
pub use job_type_yaml::JobTypeYaml;

mod priority_yaml;
pub use priority_yaml::PriorityYaml;

mod pull_request_yaml;
pub use pull_request_yaml::PullRequestYaml;

mod repository_yaml;
pub use repository_yaml::RepositoryYaml;

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
    /// Command to execute for orchestrator tasks.
    pub command: Option<String>,
    /// Perspective name for review tasks.
    pub perspective: Option<String>,
    /// Pull request for standalone PR review tasks.
    pub pull_request: Option<PullRequestYaml>,
    #[serde(default)]
    pub children: Vec<TaskNode>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}
