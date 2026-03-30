use super::{TaskNode, TaskTreeBlueprint};
use palette_domain::job::{JobType, Priority, Repository};
use palette_domain::task::{TaskId, TaskKey, TaskTree, TaskTreeNode};
use palette_domain::workflow::WorkflowId;
use std::collections::HashMap;

/// Blueprint validation error.
#[derive(Debug, palette_macros::ReasonKey)]
#[reason_namespace = "blueprint"]
pub enum BlueprintError {
    /// Task key contains invalid characters (must be `[a-z0-9-]+`).
    InvalidKey { key: String },
    /// Craft task has no review child.
    MissingReviewChild { task_key: String },
}

impl BlueprintError {
    pub fn field_path(&self) -> String {
        match self {
            BlueprintError::InvalidKey { key } => format!("tasks[key={key}].key"),
            BlueprintError::MissingReviewChild { task_key } => {
                format!("tasks[key={task_key}].children")
            }
        }
    }
}

impl TaskTreeBlueprint {
    /// Convert this Blueprint into a domain TaskTree.
    ///
    /// Task IDs are built as `{workflow_id}:{key_path}` where key_path
    /// is the `/`-separated path of task keys from root to the node.
    ///
    /// Validates key formats and structural constraints before building.
    pub fn to_task_tree(&self, workflow_id: &WorkflowId) -> Result<TaskTree, Vec<BlueprintError>> {
        let mut errors = Vec::new();
        validate_all(&self.task, &mut errors);
        if !errors.is_empty() {
            return Err(errors);
        }

        let root = &self.task;
        let root_key = TaskKey::new(&root.key);
        let root_id = TaskId::root(workflow_id, &root_key);
        let mut nodes = HashMap::new();

        insert_node(&root_id, None, None, root, &mut nodes);
        collect_nodes(
            &root_id,
            root.plan_path.as_deref(),
            &root.children,
            &mut nodes,
        );

        Ok(TaskTree::new(root_id, nodes))
    }
}

fn validate_all(node: &TaskNode, errors: &mut Vec<BlueprintError>) {
    if let Err(e) = validate_key(&node.key) {
        errors.push(e);
    }
    if let Err(e) = validate_craft_has_review(node) {
        errors.push(e);
    }
    for child in &node.children {
        validate_all(child, errors);
    }
}

fn validate_key(key: &str) -> Result<TaskKey, BlueprintError> {
    if key.is_empty()
        || !key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(BlueprintError::InvalidKey {
            key: key.to_string(),
        });
    }
    Ok(TaskKey::new(key))
}

fn validate_craft_has_review(node: &TaskNode) -> Result<(), BlueprintError> {
    if let Some(job_type) = node.job_type
        && matches!(JobType::from(job_type), JobType::Craft)
    {
        let has_review = node.children.iter().any(|c| {
            c.job_type
                .is_some_and(|jt| matches!(JobType::from(jt), JobType::Review))
        });
        if !has_review {
            return Err(BlueprintError::MissingReviewChild {
                task_key: node.key.clone(),
            });
        }
    }
    Ok(())
}

fn to_key(s: &str) -> TaskKey {
    TaskKey::new(s)
}

fn insert_node(
    task_id: &TaskId,
    parent_id: Option<&TaskId>,
    parent_plan_path: Option<&str>,
    node: &TaskNode,
    nodes: &mut HashMap<TaskId, TaskTreeNode>,
) {
    let child_ids: Vec<TaskId> = node
        .children
        .iter()
        .map(|c| task_id.child(&to_key(&c.key)))
        .collect();

    let depends_on: Vec<TaskId> = if let Some(parent) = parent_id {
        node.depends_on
            .iter()
            .map(|dep| parent.child(&TaskKey::new(dep)))
            .collect()
    } else {
        vec![]
    };

    // Inherit plan_path from parent if not explicitly set
    let plan_path = node
        .plan_path
        .clone()
        .or_else(|| parent_plan_path.map(String::from));

    nodes.insert(
        task_id.clone(),
        TaskTreeNode {
            id: task_id.clone(),
            parent_id: parent_id.cloned(),
            key: to_key(&node.key),
            plan_path,
            job_type: node.job_type.map(JobType::from),
            priority: node.priority.map(Priority::from),
            repository: node.repository.clone().map(Repository::from),
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
) {
    for child in children {
        let child_task_id = parent_task_id.child(&to_key(&child.key));
        let child_plan_path = child.plan_path.as_deref().or(parent_plan_path);

        insert_node(
            &child_task_id,
            Some(parent_task_id),
            parent_plan_path,
            child,
            nodes,
        );

        if !child.children.is_empty() {
            collect_nodes(&child_task_id, child_plan_path, &child.children, nodes);
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
        assert!(matches!(&errors[0], BlueprintError::InvalidKey { key } if key == "Feature_X"));
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
