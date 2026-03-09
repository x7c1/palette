use chrono::{DateTime, Utc};
use std::fmt;
use std::str::FromStr;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// --- Newtype IDs ---

/// Task identifier (e.g., "W-XXXXXXXX" for work, "R-XXXXXXXX" for review).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
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

/// Agent identifier for both leaders and members (e.g., "leader-1", "member-a").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct AgentId(String);

impl AgentId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate the next member ID from a sequence number
    /// (0 → "member-a", 25 → "member-z", 26 → "member-aa", ...).
    pub fn next_member(sequence: usize) -> Self {
        let suffix = member_id_suffix(sequence);
        Self(format!("member-{suffix}"))
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for AgentId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

fn member_id_suffix(n: usize) -> String {
    let mut n = n;
    let mut result = String::new();
    loop {
        result.insert(0, (b'a' + (n % 26) as u8) as char);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    result
}

// --- Enums ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
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

// --- Domain models ---

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Repository {
    pub name: String,
    pub branch: Option<String>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Task {
    pub id: TaskId,
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub task_type: TaskType,
    pub title: String,
    pub description: Option<String>,
    pub assignee: Option<AgentId>,
    pub status: TaskStatus,
    pub priority: Option<Priority>,
    pub repositories: Option<Vec<Repository>>,
    pub pr_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub notes: Option<String>,
    pub assigned_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CreateTaskRequest {
    pub id: Option<TaskId>,
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub task_type: TaskType,
    pub title: String,
    pub description: Option<String>,
    pub assignee: Option<AgentId>,
    pub priority: Option<Priority>,
    pub repositories: Option<Vec<Repository>>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub depends_on: Vec<TaskId>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct UpdateTaskRequest {
    pub id: TaskId,
    pub status: TaskStatus,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ReviewSubmission {
    pub id: i64,
    pub review_task_id: TaskId,
    pub round: i32,
    pub verdict: Verdict,
    pub summary: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ReviewComment {
    pub id: i64,
    pub submission_id: i64,
    pub file: String,
    pub line: i32,
    pub body: String,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SubmitReviewRequest {
    pub verdict: Verdict,
    pub summary: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub comments: Vec<ReviewCommentInput>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ReviewCommentInput {
    pub file: String,
    pub line: i32,
    pub body: String,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TaskFilter {
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub task_type: Option<TaskType>,
    pub status: Option<TaskStatus>,
    pub assignee: Option<AgentId>,
}

/// Side effects produced by the rule engine after a state transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleEffect {
    /// A task's status was changed by the rule engine.
    StatusChanged {
        task_id: TaskId,
        new_status: TaskStatus,
    },
    /// The review loop exceeded the max rounds; escalate.
    Escalated { task_id: TaskId, round: i32 },
    /// A task is ready to be assigned to a member (orchestrator should spawn member).
    AutoAssign { task_id: TaskId },
    /// A member's task is done; orchestrator should destroy its container.
    DestroyMember { member_id: AgentId },
}
