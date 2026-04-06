use super::{InputError, JobType, Priority, Repository};
use palette_domain::job::{CreateJobRequest as DomainCreateJobRequest, JobDetail, PlanPath, Title};
use palette_domain::task::TaskId;
use palette_domain::worker::WorkerId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateJobRequest {
    pub task_id: String,
    #[serde(rename = "type")]
    pub job_type: JobType,
    pub title: String,
    pub plan_path: String,
    pub assignee_id: Option<String>,
    pub priority: Option<Priority>,
    pub repository: Option<Repository>,
    pub command: Option<String>,
}

impl CreateJobRequest {
    pub fn validate(&self) -> Result<DomainCreateJobRequest, Vec<InputError>> {
        let job_type: palette_domain::job::JobType = self.job_type.into();

        // Build and validate detail outside the macro since it involves
        // cross-field logic (repository required for craft).
        let mut detail_errors: Vec<InputError> = Vec::new();

        let parsed_repo = self.repository.clone().map(|r| r.parse()).transpose();

        let detail = match job_type {
            palette_domain::job::JobType::Craft => match parsed_repo {
                Ok(Some(repo)) => Some(JobDetail::Craft { repository: repo }),
                Ok(None) => {
                    detail_errors.push(InputError {
                        location: palette_core::Location::Body,
                        hint: "repository".into(),
                        reason: "repository/required_for_craft".into(),
                    });
                    None
                }
                Err(e) => {
                    detail_errors.push(InputError {
                        location: palette_core::Location::Body,
                        hint: "repository".into(),
                        reason: palette_core::ReasonKey::reason_key(&e),
                    });
                    None
                }
            },
            palette_domain::job::JobType::Review => Some(JobDetail::Review { perspective: None }),
            palette_domain::job::JobType::ReviewIntegrate => Some(JobDetail::ReviewIntegrate),
            palette_domain::job::JobType::Orchestrator => Some(JobDetail::Orchestrator {
                command: self.command.clone(),
            }),
            palette_domain::job::JobType::Operator => Some(JobDetail::Operator),
        };

        // Use the macro for standard field validation
        let result = palette_macros::validate!(DomainCreateJobRequest::new {
            task_id: TaskId::parse(&self.task_id),
            title: Title::parse(&self.title),
            plan_path: PlanPath::parse(&self.plan_path),
            assignee_id: self.assignee_id.as_deref().map(WorkerId::parse).transpose(),
            #[plain]
            priority: self.priority.map(palette_domain::job::Priority::from),
            #[plain]
            detail: detail.unwrap_or(JobDetail::Operator),
        });

        match result {
            Ok(req) if detail_errors.is_empty() => Ok(req),
            Ok(_) => Err(detail_errors),
            Err(mut errors) => {
                errors.extend(detail_errors);
                Err(errors)
            }
        }
    }
}
