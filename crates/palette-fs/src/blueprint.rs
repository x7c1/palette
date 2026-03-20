use palette_domain::job::{JobType, Priority, Repository};
use serde::Deserialize;
use std::path::Path;

/// Read and parse a Blueprint YAML file into a TaskTreeBlueprint.
pub fn read_blueprint(path: &Path) -> Result<TaskTreeBlueprint, BlueprintReadError> {
    let yaml = std::fs::read_to_string(path).map_err(|e| BlueprintReadError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let blueprint: TaskTreeBlueprint =
        serde_yaml::from_str(&yaml).map_err(|e| BlueprintReadError::Parse {
            path: path.to_path_buf(),
            source: e,
        })?;
    Ok(blueprint)
}

#[derive(Debug)]
pub enum BlueprintReadError {
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: std::path::PathBuf,
        source: serde_yaml::Error,
    },
}

impl std::fmt::Display for BlueprintReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlueprintReadError::Io { path, source } => {
                write!(f, "failed to read blueprint '{}': {source}", path.display())
            }
            BlueprintReadError::Parse { path, source } => {
                write!(
                    f,
                    "failed to parse blueprint '{}': {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for BlueprintReadError {}

// --- YAML deserialization types ---

/// Root of a Blueprint YAML document.
#[derive(Debug, Deserialize)]
pub struct TaskTreeBlueprint {
    pub task: TaskTreeRoot,
    #[serde(default)]
    pub children: Vec<TaskNode>,
}

/// Root task identity.
#[derive(Debug, Deserialize)]
pub struct TaskTreeRoot {
    pub id: String,
    pub title: String,
    pub plan_path: Option<String>,
}

/// A node in the Blueprint's Task tree.
#[derive(Debug, Deserialize)]
pub struct TaskNode {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub plan_path: Option<String>,
    #[serde(rename = "type")]
    pub job_type: Option<JobTypeYaml>,
    pub description: Option<String>,
    pub priority: Option<PriorityYaml>,
    pub repository: Option<RepositoryYaml>,
    #[serde(default)]
    pub children: Vec<TaskNode>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobTypeYaml {
    Craft,
    Review,
}

impl From<JobTypeYaml> for JobType {
    fn from(t: JobTypeYaml) -> Self {
        match t {
            JobTypeYaml::Craft => JobType::Craft,
            JobTypeYaml::Review => JobType::Review,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorityYaml {
    High,
    Medium,
    Low,
}

impl From<PriorityYaml> for Priority {
    fn from(p: PriorityYaml) -> Self {
        match p {
            PriorityYaml::High => Priority::High,
            PriorityYaml::Medium => Priority::Medium,
            PriorityYaml::Low => Priority::Low,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepositoryYaml {
    pub name: String,
    pub branch: String,
}

impl From<RepositoryYaml> for Repository {
    fn from(r: RepositoryYaml) -> Self {
        Repository {
            name: r.name,
            branch: r.branch,
        }
    }
}

impl TaskTreeBlueprint {
    /// Convert this Blueprint into a domain TaskTree.
    pub fn to_task_tree(&self) -> palette_domain::task::TaskTree {
        use palette_domain::task::{TaskId, TaskTree, TaskTreeNode};
        use std::collections::HashMap;

        let root_id = TaskId::new(&self.task.id);
        let mut nodes = HashMap::new();

        let child_ids: Vec<TaskId> = self
            .children
            .iter()
            .map(|c| TaskId::new(format!("{}/{}", self.task.id, c.id)))
            .collect();

        nodes.insert(
            root_id.clone(),
            TaskTreeNode {
                id: root_id.clone(),
                parent_id: None,
                title: self.task.title.clone(),
                plan_path: self.task.plan_path.clone(),
                job_type: None,
                children: child_ids,
                depends_on: vec![],
            },
        );

        collect_nodes(&self.task.id, &root_id, &self.children, &mut nodes);

        TaskTree::new(root_id, nodes)
    }
}

fn collect_nodes(
    parent_id_str: &str,
    parent_task_id: &palette_domain::task::TaskId,
    children: &[TaskNode],
    nodes: &mut std::collections::HashMap<
        palette_domain::task::TaskId,
        palette_domain::task::TaskTreeNode,
    >,
) {
    use palette_domain::task::{TaskId, TaskTreeNode};

    for child in children {
        let child_id_str = format!("{parent_id_str}/{}", child.id);
        let child_task_id = TaskId::new(&child_id_str);

        let grandchild_ids: Vec<TaskId> = child
            .children
            .iter()
            .map(|gc| TaskId::new(format!("{child_id_str}/{}", gc.id)))
            .collect();

        let depends_on: Vec<TaskId> = child
            .depends_on
            .iter()
            .map(|dep| TaskId::new(format!("{parent_id_str}/{dep}")))
            .collect();

        nodes.insert(
            child_task_id.clone(),
            TaskTreeNode {
                id: child_task_id.clone(),
                parent_id: Some(parent_task_id.clone()),
                title: child.title.clone().unwrap_or_else(|| child.id.clone()),
                plan_path: child.plan_path.clone(),
                job_type: child.job_type.map(JobType::from),
                children: grandchild_ids,
                depends_on,
            },
        );

        if !child.children.is_empty() {
            collect_nodes(&child_id_str, &child_task_id, &child.children, nodes);
        }
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
        let blueprint: TaskTreeBlueprint = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(blueprint.task.id, "2026/feature-x");
        assert_eq!(blueprint.children.len(), 2);

        let planning = &blueprint.children[0];
        assert_eq!(planning.children.len(), 2);
        assert!(planning.job_type.is_none());

        let execution = &blueprint.children[1];
        assert_eq!(execution.depends_on, vec!["planning"]);
    }

    #[test]
    fn read_blueprint_from_file() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(
            &mut f,
            b"task:\n  id: test\n  title: Test\nchildren:\n  - id: a\n    type: craft\n",
        )
        .unwrap();

        let bp = read_blueprint(f.path()).unwrap();
        assert_eq!(bp.task.id, "test");
        assert_eq!(bp.children.len(), 1);
    }

    #[test]
    fn to_task_tree_builds_flat_index() {
        use palette_domain::job::JobType;
        use palette_domain::task::TaskId;

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
"#;
        let blueprint: TaskTreeBlueprint = serde_yaml::from_str(yaml).unwrap();
        let tree = blueprint.to_task_tree();

        // Root
        assert_eq!(tree.root_id(), &TaskId::new("2026/feature-x"));
        let root = tree.get(&TaskId::new("2026/feature-x")).unwrap();
        assert_eq!(root.title, "Add feature X");
        assert!(root.parent_id.is_none());
        assert_eq!(root.children.len(), 2);

        // planning (composite, no job_type)
        let planning = tree.get(&TaskId::new("2026/feature-x/planning")).unwrap();
        assert_eq!(planning.title, "planning");
        assert_eq!(
            planning.parent_id.as_ref().unwrap(),
            &TaskId::new("2026/feature-x")
        );
        assert!(planning.job_type.is_none());
        assert_eq!(planning.children.len(), 2);
        assert!(planning.depends_on.is_empty());

        // api-plan (leaf, craft)
        let api_plan = tree
            .get(&TaskId::new("2026/feature-x/planning/api-plan"))
            .unwrap();
        assert_eq!(api_plan.job_type, Some(JobType::Craft));
        assert_eq!(
            api_plan.plan_path.as_deref(),
            Some("2026/feature-x/planning/api-plan")
        );
        assert!(api_plan.depends_on.is_empty());

        // api-plan-review (leaf, review, depends on api-plan)
        let review = tree
            .get(&TaskId::new("2026/feature-x/planning/api-plan-review"))
            .unwrap();
        assert_eq!(review.job_type, Some(JobType::Review));
        assert_eq!(
            review.depends_on,
            vec![TaskId::new("2026/feature-x/planning/api-plan")]
        );

        // execution (depends on planning)
        let execution = tree.get(&TaskId::new("2026/feature-x/execution")).unwrap();
        assert_eq!(
            execution.depends_on,
            vec![TaskId::new("2026/feature-x/planning")]
        );

        // Total: root + planning + api-plan + api-plan-review + execution + api-impl = 6
        assert_eq!(tree.task_ids().count(), 6);
    }
}
