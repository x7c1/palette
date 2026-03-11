use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    High,
    Medium,
    Low,
}

impl From<Priority> for domain::job::Priority {
    fn from(p: Priority) -> Self {
        match p {
            Priority::High => domain::job::Priority::High,
            Priority::Medium => domain::job::Priority::Medium,
            Priority::Low => domain::job::Priority::Low,
        }
    }
}

impl From<domain::job::Priority> for Priority {
    fn from(p: domain::job::Priority) -> Self {
        match p {
            domain::job::Priority::High => Priority::High,
            domain::job::Priority::Medium => Priority::Medium,
            domain::job::Priority::Low => Priority::Low,
        }
    }
}
