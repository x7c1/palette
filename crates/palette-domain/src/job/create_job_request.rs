use super::{JobId, JobType, Priority, Repository};
use crate::agent::AgentId;

#[derive(Debug, Clone)]
pub struct CreateJobRequest {
    pub id: Option<JobId>,
    pub job_type: JobType,
    pub title: String,
    pub description: Option<String>,
    pub assignee: Option<AgentId>,
    pub priority: Option<Priority>,
    pub repositories: Option<Vec<Repository>>,
    pub depends_on: Vec<JobId>,
}
