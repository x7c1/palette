use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Draft,
    Ready,
    Todo,
    InProgress,
    InReview,
    Done,
    Blocked,
    Escalated,
}

impl From<TaskStatus> for domain::task::TaskStatus {
    fn from(s: TaskStatus) -> Self {
        match s {
            TaskStatus::Draft => domain::task::TaskStatus::Draft,
            TaskStatus::Ready => domain::task::TaskStatus::Ready,
            TaskStatus::Todo => domain::task::TaskStatus::Todo,
            TaskStatus::InProgress => domain::task::TaskStatus::InProgress,
            TaskStatus::InReview => domain::task::TaskStatus::InReview,
            TaskStatus::Done => domain::task::TaskStatus::Done,
            TaskStatus::Blocked => domain::task::TaskStatus::Blocked,
            TaskStatus::Escalated => domain::task::TaskStatus::Escalated,
        }
    }
}

impl From<domain::task::TaskStatus> for TaskStatus {
    fn from(s: domain::task::TaskStatus) -> Self {
        match s {
            domain::task::TaskStatus::Draft => TaskStatus::Draft,
            domain::task::TaskStatus::Ready => TaskStatus::Ready,
            domain::task::TaskStatus::Todo => TaskStatus::Todo,
            domain::task::TaskStatus::InProgress => TaskStatus::InProgress,
            domain::task::TaskStatus::InReview => TaskStatus::InReview,
            domain::task::TaskStatus::Done => TaskStatus::Done,
            domain::task::TaskStatus::Blocked => TaskStatus::Blocked,
            domain::task::TaskStatus::Escalated => TaskStatus::Escalated,
        }
    }
}
