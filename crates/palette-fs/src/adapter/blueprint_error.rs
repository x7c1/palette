use palette_domain::job::{InvalidPullRequest, InvalidRepository};
use palette_domain::task::InvalidTaskKey;

/// Blueprint validation error.
#[derive(Debug)]
pub enum BlueprintError {
    /// Task key is invalid.
    InvalidKey(InvalidTaskKey),
    /// Craft task has no review child.
    MissingReviewChild { task_key: String },
    /// Craft task has no repository.
    MissingRepository { task_key: String },
    /// Repository has invalid name or branch.
    InvalidRepository {
        task_key: String,
        cause: InvalidRepository,
    },
    /// Task depends on itself.
    SelfDependency { task_key: String },
    /// Same dependency listed more than once.
    DuplicateDependency { task_key: String, dep: String },
    /// Perspective specified on a non-review task.
    PerspectiveOnNonReview { task_key: String },
    /// Pull request has invalid owner, repo, or number.
    InvalidPullRequest {
        task_key: String,
        cause: InvalidPullRequest,
    },
    /// Perspective name not found in server configuration.
    UnknownPerspective {
        task_key: String,
        #[allow(dead_code)]
        perspective: String,
    },
}

impl BlueprintError {
    pub fn field_path(&self) -> String {
        match self {
            BlueprintError::InvalidKey(e) => match e {
                InvalidTaskKey::Empty => "tasks[].key".to_string(),
                InvalidTaskKey::InvalidFormat { key } => format!("tasks[key={key}].key"),
            },
            BlueprintError::MissingReviewChild { task_key } => {
                format!("tasks[key={task_key}].children")
            }
            BlueprintError::MissingRepository { task_key } => {
                format!("tasks[key={task_key}].repository")
            }
            BlueprintError::InvalidRepository { task_key, .. } => {
                format!("tasks[key={task_key}].repository")
            }
            BlueprintError::SelfDependency { task_key } => {
                format!("tasks[key={task_key}].depends_on")
            }
            BlueprintError::DuplicateDependency { task_key, dep } => {
                format!("tasks[key={task_key}].depends_on[{dep}]")
            }
            BlueprintError::InvalidPullRequest { task_key, .. } => {
                format!("tasks[key={task_key}].pull_request")
            }
            BlueprintError::PerspectiveOnNonReview { task_key } => {
                format!("tasks[key={task_key}].perspective")
            }
            BlueprintError::UnknownPerspective { task_key, .. } => {
                format!("tasks[key={task_key}].perspective")
            }
        }
    }

    pub fn reason_key(&self) -> String {
        use palette_core::ReasonKey;
        match self {
            BlueprintError::InvalidKey(e) => e.reason_key(),
            BlueprintError::MissingReviewChild { .. } => {
                "blueprint/missing_review_child".to_string()
            }
            BlueprintError::MissingRepository { .. } => "blueprint/missing_repository".to_string(),
            BlueprintError::InvalidRepository { cause, .. } => cause.reason_key(),
            BlueprintError::InvalidPullRequest { cause, .. } => cause.reason_key(),
            BlueprintError::SelfDependency { .. } => "blueprint/self_dependency".to_string(),
            BlueprintError::DuplicateDependency { .. } => {
                "blueprint/duplicate_dependency".to_string()
            }
            BlueprintError::PerspectiveOnNonReview { .. } => {
                "blueprint/perspective_on_non_review".to_string()
            }
            BlueprintError::UnknownPerspective { .. } => {
                "blueprint/unknown_perspective".to_string()
            }
        }
    }
}
