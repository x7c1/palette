mod job_entry;
mod job_id_input;
mod job_type_input;
mod priority_input;
mod repository_entry;

use job_entry::JobEntry;
use palette_domain::job::{CreateJobRequest, JobId, Priority, Repository};
use repository_entry::RepositoryEntry;
use serde::Deserialize;

/// Top-level YAML job definition file.
#[derive(Debug, Deserialize)]
pub struct JobFile {
    /// Default repositories inherited by all jobs unless overridden.
    #[serde(default)]
    repositories: Vec<RepositoryEntry>,
    jobs: Vec<JobEntry>,
}

impl JobFile {
    pub fn parse(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// Convert all job entries into CreateJobRequests, applying defaults.
    pub fn into_requests(self) -> Vec<CreateJobRequest> {
        let default_repos: Option<Vec<Repository>> = if self.repositories.is_empty() {
            None
        } else {
            Some(
                self.repositories
                    .iter()
                    .map(|r| Repository {
                        name: r.name.clone(),
                        branch: r.branch.clone(),
                    })
                    .collect(),
            )
        };

        self.jobs
            .into_iter()
            .map(|entry| {
                let repositories = entry
                    .repositories
                    .map(|repos| {
                        repos
                            .into_iter()
                            .map(|r| Repository {
                                name: r.name,
                                branch: r.branch,
                            })
                            .collect()
                    })
                    .or_else(|| default_repos.clone());

                CreateJobRequest {
                    id: Some(entry.id.into()),
                    job_type: entry.job_type.into(),
                    title: entry.title,
                    description: entry.description,
                    assignee: None,
                    priority: entry.priority.map(Priority::from),
                    repositories,
                    depends_on: entry.depends_on.into_iter().map(JobId::from).collect(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use palette_domain::job::{JobId, JobType};

    #[test]
    fn parse_basic_job_file() {
        let yaml = r#"
repositories:
  - name: x7c1/palette
    branch: feature/test

jobs:
  - id: C-A
    type: craft
    title: Create file A
    description: Create /home/agent/file-a.txt
    priority: high

  - id: R-A
    type: review
    title: Review file A
    depends_on: [C-A]
"#;
        let file = JobFile::parse(yaml).unwrap();
        assert_eq!(file.repositories.len(), 1);
        assert_eq!(file.repositories[0].name, "x7c1/palette");
        assert_eq!(file.jobs.len(), 2);

        let requests = file.into_requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].id, Some(JobId::new("C-A")));
        assert_eq!(requests[0].job_type, JobType::Craft);
        assert_eq!(requests[0].priority, Some(Priority::High));

        // Inherits top-level repositories
        let repos = requests[0].repositories.as_ref().unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].name, "x7c1/palette");
        assert_eq!(repos[0].branch.as_deref(), Some("feature/test"));

        // Review job also inherits
        assert_eq!(requests[1].depends_on, vec![JobId::new("C-A")]);
        assert!(requests[1].repositories.is_some());
    }

    #[test]
    fn per_job_repositories_override() {
        let yaml = r#"
repositories:
  - name: x7c1/default-repo
    branch: main

jobs:
  - id: C-A
    type: craft
    title: Job A
    repositories:
      - name: x7c1/special-repo
        branch: feature/special
"#;
        let file = JobFile::parse(yaml).unwrap();
        let requests = file.into_requests();

        let repos = requests[0].repositories.as_ref().unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].name, "x7c1/special-repo");
        assert_eq!(repos[0].branch.as_deref(), Some("feature/special"));
    }

    #[test]
    fn multi_repo_different_branches() {
        let yaml = r#"
repositories:
  - name: x7c1/frontend
    branch: feature/ui
  - name: x7c1/backend
    branch: feature/api

jobs:
  - id: C-A
    type: craft
    title: Full stack job
"#;
        let file = JobFile::parse(yaml).unwrap();
        let requests = file.into_requests();

        let repos = requests[0].repositories.as_ref().unwrap();
        assert_eq!(repos.len(), 2);
        assert_eq!(repos[0].name, "x7c1/frontend");
        assert_eq!(repos[0].branch.as_deref(), Some("feature/ui"));
        assert_eq!(repos[1].name, "x7c1/backend");
        assert_eq!(repos[1].branch.as_deref(), Some("feature/api"));
    }

    #[test]
    fn no_default_repositories() {
        let yaml = r#"
jobs:
  - id: C-A
    type: craft
    title: Job without repos
"#;
        let file = JobFile::parse(yaml).unwrap();
        let requests = file.into_requests();

        assert!(requests[0].repositories.is_none());
    }
}
