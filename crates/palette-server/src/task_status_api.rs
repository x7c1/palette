use palette_domain::TaskStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatusApi {
    Draft,
    Ready,
    Todo,
    InProgress,
    InReview,
    Done,
    Blocked,
    Escalated,
}

impl From<TaskStatusApi> for TaskStatus {
    fn from(s: TaskStatusApi) -> Self {
        match s {
            TaskStatusApi::Draft => TaskStatus::Draft,
            TaskStatusApi::Ready => TaskStatus::Ready,
            TaskStatusApi::Todo => TaskStatus::Todo,
            TaskStatusApi::InProgress => TaskStatus::InProgress,
            TaskStatusApi::InReview => TaskStatus::InReview,
            TaskStatusApi::Done => TaskStatus::Done,
            TaskStatusApi::Blocked => TaskStatus::Blocked,
            TaskStatusApi::Escalated => TaskStatus::Escalated,
        }
    }
}

impl From<TaskStatus> for TaskStatusApi {
    fn from(s: TaskStatus) -> Self {
        match s {
            TaskStatus::Draft => TaskStatusApi::Draft,
            TaskStatus::Ready => TaskStatusApi::Ready,
            TaskStatus::Todo => TaskStatusApi::Todo,
            TaskStatus::InProgress => TaskStatusApi::InProgress,
            TaskStatus::InReview => TaskStatusApi::InReview,
            TaskStatus::Done => TaskStatusApi::Done,
            TaskStatus::Blocked => TaskStatusApi::Blocked,
            TaskStatus::Escalated => TaskStatusApi::Escalated,
        }
    }
}
