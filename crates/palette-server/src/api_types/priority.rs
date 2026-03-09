use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    High,
    Medium,
    Low,
}

impl From<Priority> for domain::Priority {
    fn from(p: Priority) -> Self {
        match p {
            Priority::High => domain::Priority::High,
            Priority::Medium => domain::Priority::Medium,
            Priority::Low => domain::Priority::Low,
        }
    }
}

impl From<domain::Priority> for Priority {
    fn from(p: domain::Priority) -> Self {
        match p {
            domain::Priority::High => Priority::High,
            domain::Priority::Medium => Priority::Medium,
            domain::Priority::Low => Priority::Low,
        }
    }
}
