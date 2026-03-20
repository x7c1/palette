use super::job_type_input::JobTypeInput;
use super::priority_input::PriorityInput;
use crate::api_types::Repository;
use serde::Deserialize;

/// A node in the Blueprint's Task tree.
///
/// Each node represents a Task. A node can have:
/// - `children`: making it a Composite Task
/// - `type`: indicating a Job should be assigned (craft or review)
/// - both: a Task with a Job and child Tasks
/// - neither: a Task with no Job and no children (will need children added later)
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct TaskNode {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub plan_path: Option<String>,

    // Job fields (present when a Job should be assigned to this Task)
    #[serde(rename = "type")]
    pub job_type: Option<JobTypeInput>,
    pub description: Option<String>,
    pub priority: Option<PriorityInput>,
    pub repository: Option<Repository>,

    // Tree structure
    #[serde(default)]
    pub children: Vec<TaskNode>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// Root of a Blueprint YAML document using the Task tree format.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct TaskTreeBlueprint {
    pub task: TaskTreeRoot,
    #[serde(default)]
    pub children: Vec<TaskNode>,
}

/// Root task identity in the Task tree format.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct TaskTreeRoot {
    pub id: String,
    pub title: String,
    pub plan_path: Option<String>,
}

impl TaskTreeBlueprint {
    #[allow(dead_code)]
    pub fn parse(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_nested_task_tree() {
        let yaml = r#"
task:
  id: 2026/feature-x
  title: Add feature X

children:
  - id: planning
    children:
      - id: api-plan
        type: craft
        plan_path: 2026/feature-x/planning/api-plan
      - id: api-plan-review
        type: review
        depends_on: [api-plan]

  - id: execution
    depends_on: [planning]
    children:
      - id: api-impl
        type: craft
        plan_path: 2026/feature-x/execution/api-impl
        repository:
          name: x7c1/palette
          branch: feature/x-api-impl
      - id: api-impl-review
        type: review
        depends_on: [api-impl]
"#;
        let blueprint = TaskTreeBlueprint::parse(yaml).unwrap();
        assert_eq!(blueprint.task.id, "2026/feature-x");
        assert_eq!(blueprint.task.title, "Add feature X");
        assert_eq!(blueprint.children.len(), 2);

        // Planning is a composite task
        let planning = &blueprint.children[0];
        assert_eq!(planning.id, "planning");
        assert_eq!(planning.children.len(), 2);
        assert!(planning.job_type.is_none());

        // api-plan is a leaf task with a craft job
        let api_plan = &planning.children[0];
        assert_eq!(api_plan.id, "api-plan");
        assert!(api_plan.job_type.is_some());
        assert!(api_plan.children.is_empty());

        // execution depends on planning
        let execution = &blueprint.children[1];
        assert_eq!(execution.depends_on, vec!["planning"]);
        assert_eq!(execution.children.len(), 2);

        // api-impl has a repository
        let api_impl = &execution.children[0];
        assert!(api_impl.repository.is_some());
        let repo = api_impl.repository.as_ref().unwrap();
        assert_eq!(repo.name, "x7c1/palette");
    }

    #[test]
    fn parse_flat_leaf_tasks() {
        let yaml = r#"
task:
  id: 2026/simple
  title: Simple task

children:
  - id: impl
    type: craft
    plan_path: 2026/simple/impl
  - id: review
    type: review
    depends_on: [impl]
"#;
        let blueprint = TaskTreeBlueprint::parse(yaml).unwrap();
        assert_eq!(blueprint.children.len(), 2);
        assert_eq!(blueprint.children[0].id, "impl");
        assert_eq!(blueprint.children[1].depends_on, vec!["impl"]);
    }
}
