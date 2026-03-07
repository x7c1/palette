use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    Work,
    Review,
}

impl TaskType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskType::Work => "work",
            TaskType::Review => "review",
        }
    }
}

impl FromStr for TaskType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "work" => Ok(TaskType::Work),
            "review" => Ok(TaskType::Review),
            _ => Err(format!("invalid task type: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Draft => "draft",
            TaskStatus::Ready => "ready",
            TaskStatus::Todo => "todo",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::InReview => "in_review",
            TaskStatus::Done => "done",
            TaskStatus::Blocked => "blocked",
            TaskStatus::Escalated => "escalated",
        }
    }
}

impl FromStr for TaskStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "draft" => Ok(TaskStatus::Draft),
            "ready" => Ok(TaskStatus::Ready),
            "todo" => Ok(TaskStatus::Todo),
            "in_progress" => Ok(TaskStatus::InProgress),
            "in_review" => Ok(TaskStatus::InReview),
            "done" => Ok(TaskStatus::Done),
            "blocked" => Ok(TaskStatus::Blocked),
            "escalated" => Ok(TaskStatus::Escalated),
            _ => Err(format!("invalid task status: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    High,
    Medium,
    Low,
}

impl Priority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::High => "high",
            Priority::Medium => "medium",
            Priority::Low => "low",
        }
    }
}

impl FromStr for Priority {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "high" => Ok(Priority::High),
            "medium" => Ok(Priority::Medium),
            "low" => Ok(Priority::Low),
            _ => Err(format!("invalid priority: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Approved,
    ChangesRequested,
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Approved => "approved",
            Verdict::ChangesRequested => "changes_requested",
        }
    }
}

impl FromStr for Verdict {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "approved" => Ok(Verdict::Approved),
            "changes_requested" => Ok(Verdict::ChangesRequested),
            _ => Err(format!("invalid verdict: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub name: String,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    pub title: String,
    pub description: Option<String>,
    pub assignee: Option<String>,
    pub status: TaskStatus,
    pub priority: Option<Priority>,
    pub repositories: Option<Vec<Repository>>,
    pub pr_url: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub notes: Option<String>,
    pub assigned_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    pub title: String,
    pub description: Option<String>,
    pub assignee: Option<String>,
    pub priority: Option<Priority>,
    pub repositories: Option<Vec<Repository>>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTaskRequest {
    pub id: String,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewSubmission {
    pub id: i64,
    pub review_task_id: String,
    pub round: i32,
    pub verdict: Verdict,
    pub summary: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewComment {
    pub id: i64,
    pub submission_id: i64,
    pub file: String,
    pub line: i32,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitReviewRequest {
    pub verdict: Verdict,
    pub summary: Option<String>,
    #[serde(default)]
    pub comments: Vec<ReviewCommentInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewCommentInput {
    pub file: String,
    pub line: i32,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskFilter {
    #[serde(rename = "type")]
    pub task_type: Option<TaskType>,
    pub status: Option<TaskStatus>,
    pub assignee: Option<String>,
}

/// A queued message in the message_queue table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedMessage {
    pub id: i64,
    pub target_id: String,
    pub message: String,
    pub created_at: String,
}

/// Side effects produced by the rule engine after a state transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleEffect {
    /// A task's status was changed by the rule engine.
    StatusChanged {
        task_id: String,
        new_status: TaskStatus,
    },
    /// The review loop exceeded the max rounds; escalate.
    Escalated { task_id: String, round: i32 },
    /// A task is ready to be assigned to a member (orchestrator should spawn member).
    AutoAssign { task_id: String },
    /// A member's task is done; orchestrator should destroy its container.
    DestroyMember { member_id: String },
}
