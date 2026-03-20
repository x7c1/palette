use super::{JobId, JobType, Priority, Repository};
use crate::agent::AgentId;
use crate::task::TaskId;

#[derive(Debug, Clone)]
pub struct CreateJobRequest {
    pub id: Option<JobId>,
    pub task_id: Option<TaskId>,
    pub job_type: JobType,
    pub title: String,
    pub plan_path: String,
    pub description: Option<String>,
    pub assignee: Option<AgentId>,
    pub priority: Option<Priority>,
    pub repository: Option<Repository>,
    pub depends_on: Vec<JobId>,
}
