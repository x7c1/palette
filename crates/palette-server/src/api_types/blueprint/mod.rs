mod job_entry;
mod job_id_input;
pub mod job_type_input;
pub mod priority_input;
mod task;
pub mod task_node;

use job_entry::JobEntry;
use palette_domain::job::{CreateJobRequest, JobId, Priority};
use serde::Deserialize;

pub use task::Task;

/// A Blueprint defines a Task and its Jobs.
/// Stored and loaded via the Blueprint API.
#[derive(Debug, Deserialize)]
pub struct Blueprint {
    pub task: Task,
    jobs: Vec<JobEntry>,
}

impl Blueprint {
    pub fn parse(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// Convert all job entries into CreateJobRequests.
    pub fn into_requests(self) -> Vec<CreateJobRequest> {
        self.jobs
            .into_iter()
            .map(|entry| CreateJobRequest {
                id: Some(entry.id.into()),
                job_type: entry.job_type.into(),
                title: entry.title,
                plan_path: entry.plan_path,
                description: entry.description,
                assignee: None,
                priority: entry.priority.map(Priority::from),
                repository: entry.repository.map(Into::into),
                depends_on: entry.depends_on.into_iter().map(JobId::from).collect(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use palette_domain::job::{JobId, JobType};

    #[test]
    fn parse_basic_blueprint() {
        let yaml = r#"
task:
  id: 2026/feature-x
  title: Add feature X
  plan_path: 2026/feature-x

jobs:
  - id: C-A
    type: craft
    title: Create file A
    plan_path: 2026/feature-x/create-file-a
    description: Create /home/agent/file-a.txt
    priority: high
    repository:
      name: x7c1/palette
      branch: feature/test

  - id: R-A
    type: review
    title: Review file A
    plan_path: 2026/feature-x/review-file-a
    depends_on: [C-A]
"#;
        let blueprint = Blueprint::parse(yaml).unwrap();
        assert_eq!(blueprint.task.id, "2026/feature-x");
        assert_eq!(blueprint.task.title, "Add feature X");
        assert_eq!(blueprint.jobs.len(), 2);

        let requests = blueprint.into_requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].id, Some(JobId::new("C-A")));
        assert_eq!(requests[0].job_type, JobType::Craft);
        assert_eq!(requests[0].priority, Some(Priority::High));

        let repo = requests[0].repository.as_ref().unwrap();
        assert_eq!(repo.name, "x7c1/palette");
        assert_eq!(repo.branch, "feature/test");

        // Review job has no repository
        assert_eq!(requests[1].depends_on, vec![JobId::new("C-A")]);
        assert!(requests[1].repository.is_none());
    }

    #[test]
    fn per_job_repository() {
        let yaml = r#"
task:
  id: test/per-job
  title: Per-job repo test
  plan_path: test/per-job

jobs:
  - id: C-A
    type: craft
    title: Job A
    plan_path: test/per-job/job-a
    repository:
      name: x7c1/special-repo
      branch: feature/special
"#;
        let blueprint = Blueprint::parse(yaml).unwrap();
        let requests = blueprint.into_requests();

        let repo = requests[0].repository.as_ref().unwrap();
        assert_eq!(repo.name, "x7c1/special-repo");
        assert_eq!(repo.branch, "feature/special");
    }

    #[test]
    fn no_repository() {
        let yaml = r#"
task:
  id: test/no-repos
  title: No repos test
  plan_path: test/no-repos

jobs:
  - id: C-A
    type: craft
    title: Job without repos
    plan_path: test/no-repos/job-a
"#;
        let blueprint = Blueprint::parse(yaml).unwrap();
        let requests = blueprint.into_requests();

        assert!(requests[0].repository.is_none());
    }
}
