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

impl From<TaskStatus> for domain::TaskStatus {
    fn from(s: TaskStatus) -> Self {
        match s {
            TaskStatus::Draft => domain::TaskStatus::Draft,
            TaskStatus::Ready => domain::TaskStatus::Ready,
            TaskStatus::Todo => domain::TaskStatus::Todo,
            TaskStatus::InProgress => domain::TaskStatus::InProgress,
            TaskStatus::InReview => domain::TaskStatus::InReview,
            TaskStatus::Done => domain::TaskStatus::Done,
            TaskStatus::Blocked => domain::TaskStatus::Blocked,
            TaskStatus::Escalated => domain::TaskStatus::Escalated,
        }
    }
}

impl From<domain::TaskStatus> for TaskStatus {
    fn from(s: domain::TaskStatus) -> Self {
        match s {
            domain::TaskStatus::Draft => TaskStatus::Draft,
            domain::TaskStatus::Ready => TaskStatus::Ready,
            domain::TaskStatus::Todo => TaskStatus::Todo,
            domain::TaskStatus::InProgress => TaskStatus::InProgress,
            domain::TaskStatus::InReview => TaskStatus::InReview,
            domain::TaskStatus::Done => TaskStatus::Done,
            domain::TaskStatus::Blocked => TaskStatus::Blocked,
            domain::TaskStatus::Escalated => TaskStatus::Escalated,
        }
    }
}
