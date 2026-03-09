use palette_domain::TaskType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskTypeApi {
    Work,
    Review,
}

impl From<TaskTypeApi> for TaskType {
    fn from(t: TaskTypeApi) -> Self {
        match t {
            TaskTypeApi::Work => TaskType::Work,
            TaskTypeApi::Review => TaskType::Review,
        }
    }
}

impl From<TaskType> for TaskTypeApi {
    fn from(t: TaskType) -> Self {
        match t {
            TaskType::Work => TaskTypeApi::Work,
            TaskType::Review => TaskTypeApi::Review,
        }
    }
}
