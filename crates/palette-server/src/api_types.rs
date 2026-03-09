use chrono::{DateTime, Utc};
use palette_domain::{
    AgentId, CreateTaskRequest, Priority, Repository, ReviewComment, ReviewCommentInput,
    ReviewSubmission, SubmitReviewRequest, Task, TaskFilter, TaskId, TaskStatus, TaskType,
    UpdateTaskRequest, Verdict,
};
use serde::{Deserialize, Serialize};

// --- Request types (Deserialize) ---

#[derive(Debug, Deserialize)]
pub struct CreateTaskApi {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub task_type: TaskTypeApi,
    pub title: String,
    pub description: Option<String>,
    pub assignee: Option<String>,
    pub priority: Option<PriorityApi>,
    pub repositories: Option<Vec<RepositoryApi>>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

impl From<CreateTaskApi> for CreateTaskRequest {
    fn from(api: CreateTaskApi) -> Self {
        Self {
            id: api.id.map(TaskId::new),
            task_type: api.task_type.into(),
            title: api.title,
            description: api.description,
            assignee: api.assignee.map(AgentId::new),
            priority: api.priority.map(Priority::from),
            repositories: api.repositories.map(|repos| {
                repos
                    .into_iter()
                    .map(|r| Repository {
                        name: r.name,
                        branch: r.branch,
                    })
                    .collect()
            }),
            depends_on: api.depends_on.into_iter().map(TaskId::new).collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskApi {
    pub id: String,
    pub status: TaskStatusApi,
}

impl From<UpdateTaskApi> for UpdateTaskRequest {
    fn from(api: UpdateTaskApi) -> Self {
        Self {
            id: TaskId::new(api.id),
            status: api.status.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SubmitReviewApi {
    pub verdict: VerdictApi,
    pub summary: Option<String>,
    #[serde(default)]
    pub comments: Vec<ReviewCommentInputApi>,
}

impl From<SubmitReviewApi> for SubmitReviewRequest {
    fn from(api: SubmitReviewApi) -> Self {
        Self {
            verdict: api.verdict.into(),
            summary: api.summary,
            comments: api
                .comments
                .into_iter()
                .map(|c| ReviewCommentInput {
                    file: c.file,
                    line: c.line,
                    body: c.body,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ReviewCommentInputApi {
    pub file: String,
    pub line: i32,
    pub body: String,
}

#[derive(Debug, Deserialize)]
pub struct TaskFilterApi {
    #[serde(rename = "type")]
    pub task_type: Option<TaskTypeApi>,
    pub status: Option<TaskStatusApi>,
    pub assignee: Option<String>,
}

impl From<TaskFilterApi> for TaskFilter {
    fn from(api: TaskFilterApi) -> Self {
        Self {
            task_type: api.task_type.map(TaskType::from),
            status: api.status.map(TaskStatus::from),
            assignee: api.assignee.map(AgentId::new),
        }
    }
}

// --- Response types (Serialize) ---

#[derive(Debug, Serialize)]
pub struct TaskResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub task_type: TaskTypeApi,
    pub title: String,
    pub description: Option<String>,
    pub assignee: Option<String>,
    pub status: TaskStatusApi,
    pub priority: Option<PriorityApi>,
    pub repositories: Option<Vec<RepositoryApi>>,
    pub pr_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub notes: Option<String>,
    pub assigned_at: Option<DateTime<Utc>>,
}

impl From<Task> for TaskResponse {
    fn from(t: Task) -> Self {
        Self {
            id: t.id.to_string(),
            task_type: t.task_type.into(),
            title: t.title,
            description: t.description,
            assignee: t.assignee.map(|a| a.to_string()),
            status: t.status.into(),
            priority: t.priority.map(PriorityApi::from),
            repositories: t.repositories.map(|repos| {
                repos
                    .into_iter()
                    .map(|r| RepositoryApi {
                        name: r.name,
                        branch: r.branch,
                    })
                    .collect()
            }),
            pr_url: t.pr_url,
            created_at: t.created_at,
            updated_at: t.updated_at,
            notes: t.notes,
            assigned_at: t.assigned_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ReviewSubmissionResponse {
    pub id: i64,
    pub review_task_id: String,
    pub round: i32,
    pub verdict: VerdictApi,
    pub summary: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<ReviewSubmission> for ReviewSubmissionResponse {
    fn from(s: ReviewSubmission) -> Self {
        Self {
            id: s.id,
            review_task_id: s.review_task_id.to_string(),
            round: s.round,
            verdict: s.verdict.into(),
            summary: s.summary,
            created_at: s.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ReviewCommentResponse {
    pub id: i64,
    pub submission_id: i64,
    pub file: String,
    pub line: i32,
    pub body: String,
}

impl From<ReviewComment> for ReviewCommentResponse {
    fn from(c: ReviewComment) -> Self {
        Self {
            id: c.id,
            submission_id: c.submission_id,
            file: c.file,
            line: c.line,
            body: c.body,
        }
    }
}

// --- Shared enum types (both Serialize and Deserialize) ---

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskTypeApi {
    Work,
    Review,
}

impl From<TaskTypeApi> for TaskType {
    fn from(t: TaskTypeApi) -> Self {
        match t {
            TaskTypeApi::Work => TaskType::Work,
            TaskTypeApi::Review => TaskType::Review,
        }
    }
}

impl From<TaskType> for TaskTypeApi {
    fn from(t: TaskType) -> Self {
        match t {
            TaskType::Work => TaskTypeApi::Work,
            TaskType::Review => TaskTypeApi::Review,
        }
    }
}

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerdictApi {
    Approved,
    ChangesRequested,
}

impl From<VerdictApi> for Verdict {
    fn from(v: VerdictApi) -> Self {
        match v {
            VerdictApi::Approved => Verdict::Approved,
            VerdictApi::ChangesRequested => Verdict::ChangesRequested,
        }
    }
}

impl From<Verdict> for VerdictApi {
    fn from(v: Verdict) -> Self {
        match v {
            Verdict::Approved => VerdictApi::Approved,
            Verdict::ChangesRequested => VerdictApi::ChangesRequested,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryApi {
    pub name: String,
    pub branch: Option<String>,
}
