use super::{JobStatus, JobType};
use crate::agent::AgentId;

#[derive(Debug, Clone, Default)]
pub struct JobFilter {
    pub job_type: Option<JobType>,
    pub status: Option<JobStatus>,
    pub assignee: Option<AgentId>,
}
