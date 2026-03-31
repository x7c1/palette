use super::{TaskId, TaskKey, TaskStatus};
use crate::job::{CreateJobRequest, JobId, JobType, PlanPath, Priority, Repository, Title};
use crate::workflow::WorkflowId;
use palette_core::ReasonKey;

/// A Task is a goal to achieve. Tasks form a tree structure where a Composite
/// Task has child Tasks. A Task can also have a Job assigned to it.
///
/// Constructed by combining structural information (from Blueprint / TaskTree)
/// with execution state (from DB / TaskState).
#[derive(Debug, Clone)]
pub struct Task {
    pub id: TaskId,
    pub workflow_id: WorkflowId,
    pub parent_id: Option<TaskId>,
    pub key: TaskKey,
    pub plan_path: Option<String>,
    pub job_type: Option<JobType>,
    pub priority: Option<Priority>,
    pub repository: Option<Repository>,
    pub status: TaskStatus,
    pub children: Vec<Task>,
    pub depends_on: Vec<TaskId>,
}

impl Task {
    /// A Composite Task is a Task that has child Tasks.
    pub fn is_composite(&self) -> bool {
        !self.children.is_empty()
    }

    /// Build a CreateJobRequest from this task.
    ///
    /// Requires `job_type` and `plan_path` to be set.
    /// The task key is used as the job title.
    pub fn to_create_job_request(&self) -> Result<CreateJobRequest, TaskToJobError> {
        let job_type = self
            .job_type
            .ok_or_else(|| TaskToJobError::MissingJobType {
                task_id: self.id.clone(),
            })?;
        let plan_path_str =
            self.plan_path
                .as_deref()
                .ok_or_else(|| TaskToJobError::MissingPlanPath {
                    task_id: self.id.clone(),
                })?;
        let title =
            Title::parse(self.key.to_string()).map_err(|e| TaskToJobError::InvalidField {
                task_id: self.id.clone(),
                detail: e.reason_key(),
            })?;
        let plan_path =
            PlanPath::parse(plan_path_str).map_err(|e| TaskToJobError::InvalidField {
                task_id: self.id.clone(),
                detail: e.reason_key(),
            })?;

        Ok(CreateJobRequest::new(
            Some(JobId::generate(job_type)),
            self.id.clone(),
            job_type,
            title,
            plan_path,
            None,
            self.priority,
            self.repository.clone(),
        ))
    }
}

/// Error when converting a Task to a CreateJobRequest.
#[derive(Debug, palette_macros::ReasonKey)]
pub enum TaskToJobError {
    MissingJobType { task_id: TaskId },
    MissingPlanPath { task_id: TaskId },
    InvalidField { task_id: TaskId, detail: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_task(job_type: Option<JobType>, plan_path: Option<&str>) -> Task {
        Task {
            id: TaskId::parse("wf-test:task-1").unwrap(),
            workflow_id: WorkflowId::parse("wf-test").unwrap(),
            parent_id: None,
            key: TaskKey::parse("my-task").unwrap(),
            plan_path: plan_path.map(String::from),
            job_type,
            priority: Some(Priority::High),
            repository: None,
            status: TaskStatus::Ready,
            children: vec![],
            depends_on: vec![],
        }
    }

    #[test]
    fn creates_job_request_from_craft_task() {
        let task = test_task(Some(JobType::Craft), Some("plans/my-task"));
        let req = task.to_create_job_request().unwrap();
        assert_eq!(req.task_id, task.id);
        assert_eq!(req.job_type, JobType::Craft);
        assert_eq!(req.title.as_ref(), "my-task");
        assert_eq!(req.plan_path.as_ref(), "plans/my-task");
        assert_eq!(req.priority, Some(Priority::High));
        assert!(req.assignee_id.is_none());
    }

    #[test]
    fn creates_job_request_from_review_task() {
        let task = test_task(Some(JobType::Review), Some("plans/review"));
        let req = task.to_create_job_request().unwrap();
        assert_eq!(req.job_type, JobType::Review);
    }

    #[test]
    fn fails_without_job_type() {
        let task = test_task(None, Some("plans/task"));
        let err = task.to_create_job_request().unwrap_err();
        assert!(matches!(err, TaskToJobError::MissingJobType { .. }));
    }

    #[test]
    fn fails_without_plan_path() {
        let task = test_task(Some(JobType::Craft), None);
        let err = task.to_create_job_request().unwrap_err();
        assert!(matches!(err, TaskToJobError::MissingPlanPath { .. }));
    }
}
