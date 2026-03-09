use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    Work,
    Review,
}

impl From<TaskType> for domain::TaskType {
    fn from(t: TaskType) -> Self {
        match t {
            TaskType::Work => domain::TaskType::Work,
            TaskType::Review => domain::TaskType::Review,
        }
    }
}

impl From<domain::TaskType> for TaskType {
    fn from(t: domain::TaskType) -> Self {
        match t {
            domain::TaskType::Work => TaskType::Work,
            domain::TaskType::Review => TaskType::Review,
        }
    }
}
