use palette_domain as domain;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobType {
    Craft,
    Review,
    ReviewIntegrate,
    Orchestrator,
    Operator,
}

impl From<JobType> for domain::job::JobType {
    fn from(t: JobType) -> Self {
        match t {
            JobType::Craft => domain::job::JobType::Craft,
            JobType::Review => domain::job::JobType::Review,
            JobType::ReviewIntegrate => domain::job::JobType::ReviewIntegrate,
            JobType::Orchestrator => domain::job::JobType::Orchestrator,
            JobType::Operator => domain::job::JobType::Operator,
        }
    }
}

impl From<domain::job::JobType> for JobType {
    fn from(t: domain::job::JobType) -> Self {
        match t {
            domain::job::JobType::Craft => JobType::Craft,
            domain::job::JobType::Review => JobType::Review,
            domain::job::JobType::ReviewIntegrate => JobType::ReviewIntegrate,
            domain::job::JobType::Orchestrator => JobType::Orchestrator,
            domain::job::JobType::Operator => JobType::Operator,
        }
    }
}
