use palette_domain::task::TaskType;
use serde::Deserialize;

/// Task type as represented in YAML input.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TaskTypeInput {
    Work,
    Review,
}

impl From<TaskTypeInput> for TaskType {
    fn from(t: TaskTypeInput) -> Self {
        match t {
            TaskTypeInput::Work => TaskType::Work,
            TaskTypeInput::Review => TaskType::Review,
        }
    }
}
