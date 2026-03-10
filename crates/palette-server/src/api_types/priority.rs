use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    High,
    Medium,
    Low,
}

impl From<Priority> for domain::task::Priority {
    fn from(p: Priority) -> Self {
        match p {
            Priority::High => domain::task::Priority::High,
            Priority::Medium => domain::task::Priority::Medium,
            Priority::Low => domain::task::Priority::Low,
        }
    }
}

impl From<domain::task::Priority> for Priority {
    fn from(p: domain::task::Priority) -> Self {
        match p {
            domain::task::Priority::High => Priority::High,
            domain::task::Priority::Medium => Priority::Medium,
            domain::task::Priority::Low => Priority::Low,
        }
    }
}
