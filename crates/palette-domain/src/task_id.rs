use std::fmt;

use crate::task_type::TaskType;

/// Task identifier (e.g., "W-XXXXXXXX" for work, "R-XXXXXXXX" for review).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId(String);

impl TaskId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn generate(task_type: TaskType) -> Self {
        let prefix = match task_type {
            TaskType::Work => 'W',
            TaskType::Review => 'R',
        };
        let suffix = &uuid::Uuid::new_v4().as_simple().to_string()[..8];
        Self(format!("{prefix}-{suffix}"))
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for TaskId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
