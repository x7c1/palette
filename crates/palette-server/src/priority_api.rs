use palette_domain::Priority;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorityApi {
    High,
    Medium,
    Low,
}

impl From<PriorityApi> for Priority {
    fn from(p: PriorityApi) -> Self {
        match p {
            PriorityApi::High => Priority::High,
            PriorityApi::Medium => Priority::Medium,
            PriorityApi::Low => Priority::Low,
        }
    }
}

impl From<Priority> for PriorityApi {
    fn from(p: Priority) -> Self {
        match p {
            Priority::High => PriorityApi::High,
            Priority::Medium => PriorityApi::Medium,
            Priority::Low => PriorityApi::Low,
        }
    }
}
