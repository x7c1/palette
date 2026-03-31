use super::{TaskNode, TaskTreeBlueprint};
use palette_domain::job::{InvalidRepository, JobType, Priority, Repository};
use palette_domain::task::{InvalidTaskKey, TaskId, TaskKey, TaskTree, TaskTreeNode};
use palette_domain::workflow::WorkflowId;
use std::collections::HashMap;

/// Blueprint validation error.
#[derive(Debug)]
pub enum BlueprintError {
    /// Task key is invalid.
    InvalidKey(InvalidTaskKey),
    /// Craft task has no review child.
    MissingReviewChild { task_key: String },
    /// Repository has invalid name or branch.
    InvalidRepository {
        task_key: String,
        cause: InvalidRepository,
    },
}

impl BlueprintError {
    pub fn field_path(&self) -> String {
        match self {
            BlueprintError::InvalidKey(e) => match e {
                InvalidTaskKey::Empty => "tasks[].key".to_string(),
                InvalidTaskKey::InvalidFormat { key } => format!("tasks[key={key}].key"),
            },
            BlueprintError::MissingReviewChild { task_key } => {
                format!("tasks[key={task_key}].children")
            }
            BlueprintError::InvalidRepository { task_key, .. } => {
                format!("tasks[key={task_key}].repository")
            }
        }
    }

    pub fn reason_key(&self) -> String {
        use palette_core::ReasonKey;
        match self {
            BlueprintError::InvalidKey(e) => e.reason_key(),
            BlueprintError::MissingReviewChild { .. } => {
                "blueprint/missing_review_child".to_string()
            }
            BlueprintError::InvalidRepository { cause, .. } => cause.reason_key(),
        }
    }
}

/// Result of validating a Blueprint node tree.
struct Validated<'a> {
    keys: HashMap<&'a str, TaskKey>,
}

impl TaskTreeBlueprint {
    /// Convert this Blueprint into a domain TaskTree.
    ///
    /// Task IDs are built as `{workflow_id}:{key_path}` where key_path
    /// is the `/`-separated path of task keys from root to the node.
    ///
    /// Validates all constraints first (collecting all errors), then builds
    /// the tree using the validated keys.
    pub fn to_task_tree(&self, workflow_id: &WorkflowId) -> Result<TaskTree, Vec<BlueprintError>> {
        let validated = validate_tree(&self.task)?;

        let root = &self.task;
        let root_key = &validated.keys[root.key.as_str()];
        let root_id = TaskId::root(workflow_id, root_key);
        let mut nodes = HashMap::new();

        insert_node(&root_id, None, None, root, &mut nodes, &validated.keys);
        collect_nodes(
            &root_id,
            root.plan_path.as_deref(),
            &root.children,
            &mut nodes,
            &validated.keys,
        );

        Ok(TaskTree::new(root_id, nodes))
    }
}

/// Validate all nodes recursively. Returns parsed keys on success,
/// or all collected errors on failure.
fn validate_tree(root: &TaskNode) -> Result<Validated<'_>, Vec<BlueprintError>> {
    let (errors, keys) = validate_node(root);
    if errors.is_empty() {
        Ok(Validated { keys })
    } else {
        Err(errors)
    }
}

/// Validate a single node and all its descendants.
/// Returns (errors, parsed_keys) so callers can aggregate.
fn validate_node(node: &TaskNode) -> (Vec<BlueprintError>, HashMap<&str, TaskKey>) {
    let mut errors = Vec::new();
    let mut keys = HashMap::new();

    match TaskKey::parse(&node.key) {
        Ok(k) => {
            keys.insert(node.key.as_str(), k);
        }
        Err(e) => {
            errors.push(BlueprintError::InvalidKey(e));
        }
    }

    if let Some(job_type) = node.job_type
        && matches!(JobType::from(job_type), JobType::Craft)
        && !node.children.iter().any(|c| {
            c.job_type
                .is_some_and(|jt| matches!(JobType::from(jt), JobType::Review))
        })
    {
        errors.push(BlueprintError::MissingReviewChild {
            task_key: node.key.clone(),
        });
    }

    if let Some(ref repo) = node.repository
        && let Err(cause) = Repository::parse(&repo.name, &repo.branch)
    {
        errors.push(BlueprintError::InvalidRepository {
            task_key: node.key.clone(),
            cause,
        });
    }

    for dep in &node.depends_on {
        if !keys.contains_key(dep.as_str()) {
            match TaskKey::parse(dep) {
                Ok(k) => {
                    keys.insert(dep.as_str(), k);
                }
                Err(e) => {
                    errors.push(BlueprintError::InvalidKey(e));
                }
            }
        }
    }

    for child in &node.children {
        let (child_errors, child_keys) = validate_node(child);
        errors.extend(child_errors);
        keys.extend(child_keys);
    }

    (errors, keys)
}

fn insert_node(
    task_id: &TaskId,
    parent_id: Option<&TaskId>,
    parent_plan_path: Option<&str>,
    node: &TaskNode,
    nodes: &mut HashMap<TaskId, TaskTreeNode>,
    keys: &HashMap<&str, TaskKey>,
) {
    let key = keys[node.key.as_str()].clone();

    let child_ids: Vec<TaskId> = node
        .children
        .iter()
        .map(|c| task_id.child(&keys[c.key.as_str()]))
        .collect();

    let depends_on: Vec<TaskId> = if let Some(parent) = parent_id {
        node.depends_on
            .iter()
            .map(|dep| parent.child(&keys[dep.as_str()]))
            .collect()
    } else {
        vec![]
    };

    let plan_path = node
        .plan_path
        .clone()
        .or_else(|| parent_plan_path.map(String::from));

    nodes.insert(
        task_id.clone(),
        TaskTreeNode {
            id: task_id.clone(),
            parent_id: parent_id.cloned(),
            key,
            plan_path,
            job_type: node.job_type.map(JobType::from),
            priority: node.priority.map(Priority::from),
            repository: node.repository.clone().and_then(|r| r.parse().ok()),
            children: child_ids,
            depends_on,
        },
    );
}

fn collect_nodes(
    parent_task_id: &TaskId,
    parent_plan_path: Option<&str>,
    children: &[TaskNode],
    nodes: &mut HashMap<TaskId, TaskTreeNode>,
    keys: &HashMap<&str, TaskKey>,
) {
    for child in children {
        let child_task_id = parent_task_id.child(&keys[child.key.as_str()]);
        let child_plan_path = child.plan_path.as_deref().or(parent_plan_path);

        insert_node(
            &child_task_id,
            Some(parent_task_id),
            parent_plan_path,
            child,
            nodes,
            keys,
        );

        if !child.children.is_empty() {
            collect_nodes(
                &child_task_id,
                child_plan_path,
                &child.children,
                nodes,
                keys,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TaskTreeBlueprint;
    use super::*;

    #[test]
    fn builds_flat_index_from_nested_blueprint() {
        let wf_id = WorkflowId::parse("wf-test").unwrap();
        let yaml = r#"
task:
  key: feature-x
  children:
    - key: planning
      children:
        - key: api-plan
          type: craft
          plan_path: planning/api-plan
          children:
            - key: api-plan-review
              type: review

    - key: execution
      depends_on: [planning]
      children:
        - key: api-impl
          type: craft
          plan_path: execution/api-impl
          children:
            - key: api-impl-review
              type: review
"#;
        let blueprint: TaskTreeBlueprint = serde_yaml::from_str(yaml).unwrap();
        let tree = blueprint.to_task_tree(&wf_id).unwrap();

        // Root
        assert_eq!(tree.root_id(), &TaskId::parse("wf-test:feature-x").unwrap());
        let root = tree.find_by_key("feature-x").unwrap();
        assert_eq!(root.key, "feature-x");
        assert!(root.parent_id.is_none());
        assert_eq!(root.children.len(), 2);

        // planning (composite, no job_type)
        let planning = tree.find_by_key("planning").unwrap();
        assert_eq!(
            planning.id,
            TaskId::parse("wf-test:feature-x/planning").unwrap()
        );
        assert_eq!(planning.parent_id.as_ref().unwrap(), tree.root_id());
        assert!(planning.job_type.is_none());
        assert_eq!(planning.children.len(), 1);
        assert!(planning.depends_on.is_empty());

        // api-plan (composite craft with review child)
        let api_plan = tree.find_by_key("api-plan").unwrap();
        assert_eq!(api_plan.job_type, Some(JobType::Craft));
        assert_eq!(api_plan.plan_path.as_deref(), Some("planning/api-plan"));
        assert!(api_plan.depends_on.is_empty());
        assert_eq!(api_plan.children.len(), 1);

        // api-plan-review (child of api-plan, review type, inherits plan_path)
        let review = tree.find_by_key("api-plan-review").unwrap();
        assert_eq!(review.job_type, Some(JobType::Review));
        assert_eq!(review.parent_id.as_ref().unwrap(), &api_plan.id);
        assert_eq!(
            review.plan_path.as_deref(),
            Some("planning/api-plan"),
            "review should inherit plan_path from parent craft"
        );
        assert!(review.depends_on.is_empty());

        // execution (depends on planning)
        let execution = tree.find_by_key("execution").unwrap();
        assert_eq!(execution.depends_on, vec![planning.id.clone()]);

        // Total: root + planning + api-plan + api-plan-review + execution + api-impl + api-impl-review = 7
        assert_eq!(tree.task_ids().count(), 7);
    }

    #[test]
    fn rejects_invalid_key_format() {
        let wf_id = WorkflowId::parse("wf-test").unwrap();
        let yaml = r#"
task:
  key: Feature_X
  children:
    - key: valid-key
"#;
        let blueprint: TaskTreeBlueprint = serde_yaml::from_str(yaml).unwrap();
        let errors = blueprint.to_task_tree(&wf_id).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            &errors[0],
            BlueprintError::InvalidKey(InvalidTaskKey::InvalidFormat { key }) if key == "Feature_X"
        ));
    }

    #[test]
    fn rejects_craft_without_review_child() {
        let wf_id = WorkflowId::parse("wf-test").unwrap();
        let yaml = r#"
task:
  key: root
  children:
    - key: my-craft
      type: craft
"#;
        let blueprint: TaskTreeBlueprint = serde_yaml::from_str(yaml).unwrap();
        let errors = blueprint.to_task_tree(&wf_id).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], BlueprintError::MissingReviewChild { task_key } if task_key == "my-craft")
        );
    }

    #[test]
    fn collects_multiple_errors() {
        let wf_id = WorkflowId::parse("wf-test").unwrap();
        let yaml = r#"
task:
  key: INVALID
  children:
    - key: craft-no-review
      type: craft
"#;
        let blueprint: TaskTreeBlueprint = serde_yaml::from_str(yaml).unwrap();
        let errors = blueprint.to_task_tree(&wf_id).unwrap_err();
        assert_eq!(errors.len(), 2);
    }
}
