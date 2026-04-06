use super::{JobType, Repository};

/// Job-type-specific fields, replacing the flat `job_type` / `repository` / `command`
/// combination. Each variant carries only the fields relevant to that job type.
#[derive(Debug, Clone)]
pub enum JobDetail {
    Craft { repository: Repository },
    Review { perspective: Option<String> },
    ReviewIntegrate,
    Orchestrator { command: Option<String> },
    Operator,
}

impl JobDetail {
    /// Derive the [`JobType`] from this detail variant.
    pub fn job_type(&self) -> JobType {
        match self {
            JobDetail::Craft { .. } => JobType::Craft,
            JobDetail::Review { .. } => JobType::Review,
            JobDetail::ReviewIntegrate => JobType::ReviewIntegrate,
            JobDetail::Orchestrator { .. } => JobType::Orchestrator,
            JobDetail::Operator => JobType::Operator,
        }
    }

    /// Return the repository if this variant carries one.
    pub fn repository(&self) -> Option<&Repository> {
        match self {
            JobDetail::Craft { repository } => Some(repository),
            JobDetail::Review { .. }
            | JobDetail::ReviewIntegrate
            | JobDetail::Orchestrator { .. }
            | JobDetail::Operator => None,
        }
    }

    /// Return the perspective name if this variant carries one.
    pub fn perspective(&self) -> Option<&str> {
        match self {
            JobDetail::Review { perspective } => perspective.as_deref(),
            JobDetail::Craft { .. }
            | JobDetail::ReviewIntegrate
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
            | JobDetail::ReviewIntegrate
            | JobDetail::Operator => None,
        }
    }
}
