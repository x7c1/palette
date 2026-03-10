use super::priority_input::PriorityInput;
use super::repository_entry::RepositoryEntry;
use super::task_id_input::TaskIdInput;
use super::task_type_input::TaskTypeInput;
use serde::Deserialize;

/// A single task entry in the YAML file.
#[derive(Debug, Deserialize)]
pub(super) struct TaskEntry {
    pub id: TaskIdInput,
    #[serde(rename = "type")]
    pub task_type: TaskTypeInput,
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<PriorityInput>,
    /// Per-task repositories override. If omitted, inherits from top-level.
    pub repositories: Option<Vec<RepositoryEntry>>,
    #[serde(default)]
    pub depends_on: Vec<TaskIdInput>,
}
