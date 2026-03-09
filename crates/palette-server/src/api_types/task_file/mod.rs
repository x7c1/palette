mod priority_input;
mod repository_entry;
mod task_entry;
mod task_id_input;
mod task_type_input;

use palette_domain::{CreateTaskRequest, Priority, Repository, TaskId};
use repository_entry::RepositoryEntry;
use serde::Deserialize;
use task_entry::TaskEntry;

/// Top-level YAML task definition file.
#[derive(Debug, Deserialize)]
pub struct TaskFile {
    /// Default repositories inherited by all tasks unless overridden.
    #[serde(default)]
    repositories: Vec<RepositoryEntry>,
    tasks: Vec<TaskEntry>,
}

impl TaskFile {
    pub fn parse(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// Convert all task entries into CreateTaskRequests, applying defaults.
    pub fn into_requests(self) -> Vec<CreateTaskRequest> {
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

        self.tasks
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

                CreateTaskRequest {
                    id: Some(entry.id.into()),
                    task_type: entry.task_type.into(),
                    title: entry.title,
                    description: entry.description,
                    assignee: None,
                    priority: entry.priority.map(Priority::from),
                    repositories,
                    depends_on: entry.depends_on.into_iter().map(TaskId::from).collect(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use palette_domain::{TaskId, TaskType};

    #[test]
    fn parse_basic_task_file() {
        let yaml = r#"
repositories:
  - name: x7c1/palette
    branch: feature/test

tasks:
  - id: W-A
    type: work
    title: Create file A
    description: Create /home/agent/file-a.txt
    priority: high

  - id: R-A
    type: review
    title: Review file A
    depends_on: [W-A]
"#;
        let file = TaskFile::parse(yaml).unwrap();
        assert_eq!(file.repositories.len(), 1);
        assert_eq!(file.repositories[0].name, "x7c1/palette");
        assert_eq!(file.tasks.len(), 2);

        let requests = file.into_requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].id, Some(TaskId::new("W-A")));
        assert_eq!(requests[0].task_type, TaskType::Work);
        assert_eq!(requests[0].priority, Some(Priority::High));

        // Inherits top-level repositories
        let repos = requests[0].repositories.as_ref().unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].name, "x7c1/palette");
        assert_eq!(repos[0].branch.as_deref(), Some("feature/test"));

        // Review task also inherits
        assert_eq!(requests[1].depends_on, vec![TaskId::new("W-A")]);
        assert!(requests[1].repositories.is_some());
    }

    #[test]
    fn per_task_repositories_override() {
        let yaml = r#"
repositories:
  - name: x7c1/default-repo
    branch: main

tasks:
  - id: W-A
    type: work
    title: Task A
    repositories:
      - name: x7c1/special-repo
        branch: feature/special
"#;
        let file = TaskFile::parse(yaml).unwrap();
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

tasks:
  - id: W-A
    type: work
    title: Full stack task
"#;
        let file = TaskFile::parse(yaml).unwrap();
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
tasks:
  - id: W-A
    type: work
    title: Task without repos
"#;
        let file = TaskFile::parse(yaml).unwrap();
        let requests = file.into_requests();

        assert!(requests[0].repositories.is_none());
    }
}
