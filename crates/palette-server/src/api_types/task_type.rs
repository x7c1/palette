use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    Work,
    Review,
}

impl From<TaskType> for domain::task::TaskType {
    fn from(t: TaskType) -> Self {
        match t {
            TaskType::Work => domain::task::TaskType::Work,
            TaskType::Review => domain::task::TaskType::Review,
        }
    }
}

impl From<domain::task::TaskType> for TaskType {
    fn from(t: domain::task::TaskType) -> Self {
        match t {
            domain::task::TaskType::Work => TaskType::Work,
            domain::task::TaskType::Review => TaskType::Review,
        }
    }
}
