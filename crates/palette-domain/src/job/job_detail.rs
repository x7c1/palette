use super::{JobType, Repository};

/// Job-type-specific fields, replacing the flat `job_type` / `repository` / `command`
/// combination. Each variant carries only the fields relevant to that job type.
#[derive(Debug, Clone)]
pub enum JobDetail {
    Craft { repository: Repository },
    Review,
    ReviewIntegrate,
    Orchestrator { command: Option<String> },
    Operator,
}

impl JobDetail {
    /// Derive the [`JobType`] from this detail variant.
    pub fn job_type(&self) -> JobType {
        match self {
            JobDetail::Craft { .. } => JobType::Craft,
            JobDetail::Review => JobType::Review,
            JobDetail::ReviewIntegrate => JobType::ReviewIntegrate,
            JobDetail::Orchestrator { .. } => JobType::Orchestrator,
            JobDetail::Operator => JobType::Operator,
        }
    }
}
