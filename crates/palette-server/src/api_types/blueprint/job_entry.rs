use super::job_id_input::JobIdInput;
use super::job_type_input::JobTypeInput;
use super::priority_input::PriorityInput;
use super::repository_entry::RepositoryEntry;
use serde::Deserialize;

/// A single job entry in the YAML file.
#[derive(Debug, Deserialize)]
pub(super) struct JobEntry {
    pub id: JobIdInput,
    #[serde(rename = "type")]
    pub job_type: JobTypeInput,
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<PriorityInput>,
    pub repository: Option<RepositoryEntry>,
    #[serde(default)]
    pub depends_on: Vec<JobIdInput>,
}
