use super::JobType;
use crate::agent::AgentId;

#[derive(Debug, Clone, Default)]
pub struct JobFilter {
    pub job_type: Option<JobType>,
    /// Status filter as a raw string (e.g., "in_progress", "todo").
    /// This matches against the DB column directly, regardless of job type.
    pub status: Option<String>,
    pub assignee: Option<AgentId>,
}
