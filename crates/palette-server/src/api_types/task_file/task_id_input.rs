use palette_domain::task::TaskId;
use serde::Deserialize;

/// Task ID as represented in YAML input.
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub(super) struct TaskIdInput(pub String);

impl From<TaskIdInput> for TaskId {
    fn from(id: TaskIdInput) -> Self {
        TaskId::new(id.0)
    }
}
