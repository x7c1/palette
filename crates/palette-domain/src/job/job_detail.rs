use super::{JobType, PerspectiveName, PullRequest, Repository, ReviewTarget};

/// Job-type-specific fields, replacing the flat `job_type` / `repository` / `command`
/// combination. Each variant carries only the fields relevant to that job type.
#[derive(Debug, Clone)]
pub enum JobDetail {
    Craft {
        repository: Repository,
    },
    Review {
        perspective: Option<PerspectiveName>,
        target: ReviewTarget,
    },
    ReviewIntegrate {
        target: ReviewTarget,
    },
    Orchestrator {
        command: Option<String>,
    },
    Operator,
}

impl JobDetail {
    /// Derive the [`JobType`] from this detail variant.
    pub fn job_type(&self) -> JobType {
        match self {
            JobDetail::Craft { .. } => JobType::Craft,
            JobDetail::Review { .. } => JobType::Review,
            JobDetail::ReviewIntegrate { .. } => JobType::ReviewIntegrate,
            JobDetail::Orchestrator { .. } => JobType::Orchestrator,
            JobDetail::Operator => JobType::Operator,
        }
    }

    /// Return the repository if this variant carries one.
    pub fn repository(&self) -> Option<&Repository> {
        match self {
            JobDetail::Craft { repository } => Some(repository),
            JobDetail::Review { .. }
            | JobDetail::ReviewIntegrate { .. }
            | JobDetail::Orchestrator { .. }
            | JobDetail::Operator => None,
        }
    }

    /// Return the perspective name if this variant carries one.
    pub fn perspective(&self) -> Option<&PerspectiveName> {
        match self {
            JobDetail::Review { perspective, .. } => perspective.as_ref(),
            JobDetail::Craft { .. }
            | JobDetail::ReviewIntegrate { .. }
            | JobDetail::Orchestrator { .. }
            | JobDetail::Operator => None,
        }
    }

    /// Return the command if this variant carries one.
    pub fn command(&self) -> Option<&str> {
        match self {
            JobDetail::Orchestrator { command } => command.as_deref(),
            JobDetail::Craft { .. }
            | JobDetail::Review { .. }
            | JobDetail::ReviewIntegrate { .. }
            | JobDetail::Operator => None,
        }
    }

    /// Return the review target if this is a Review or ReviewIntegrate variant.
    pub fn review_target(&self) -> Option<&ReviewTarget> {
        match self {
            JobDetail::Review { target, .. } | JobDetail::ReviewIntegrate { target } => {
                Some(target)
            }
            _ => None,
        }
    }

    /// Return the pull request if this is a PR review.
    pub fn pull_request(&self) -> Option<&PullRequest> {
        self.review_target().and_then(ReviewTarget::pull_request)
    }
}
