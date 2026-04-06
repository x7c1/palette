use crate::blueprint::{TaskNode, TaskTreeBlueprint};
use palette_domain::job::{InvalidRepository, JobDetail, Priority};
use palette_domain::task::{InvalidTaskKey, TaskId, TaskKey, TaskTree, TaskTreeNode};
use palette_domain::workflow::WorkflowId;
use std::collections::{HashMap, HashSet};

use super::blueprint_validator::BlueprintValidator;

/// Blueprint validation error.
#[derive(Debug)]
pub enum BlueprintError {
    /// Task key is invalid.
    InvalidKey(InvalidTaskKey),
    /// Craft task has no review child.
    MissingReviewChild { task_key: String },
    /// Craft task has no repository.
    MissingRepository { task_key: String },
    /// Repository has invalid name or branch.
    InvalidRepository {
        task_key: String,
        cause: InvalidRepository,
    },
    /// Task depends on itself.
    SelfDependency { task_key: String },
    /// Same dependency listed more than once.
    DuplicateDependency { task_key: String, dep: String },
    /// Perspective specified on a non-review task.
    PerspectiveOnNonReview { task_key: String },
    /// Perspective name not found in server configuration.
    UnknownPerspective {
        task_key: String,
        #[allow(dead_code)]
        perspective: String,
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
            BlueprintError::MissingRepository { task_key } => {
                format!("tasks[key={task_key}].repository")
            }
            BlueprintError::InvalidRepository { task_key, .. } => {
                format!("tasks[key={task_key}].repository")
            }
            BlueprintError::SelfDependency { task_key } => {
                format!("tasks[key={task_key}].depends_on")
            }
            BlueprintError::DuplicateDependency { task_key, dep } => {
                format!("tasks[key={task_key}].depends_on[{dep}]")
            }
            BlueprintError::PerspectiveOnNonReview { task_key } => {
                format!("tasks[key={task_key}].perspective")
            }
            BlueprintError::UnknownPerspective { task_key, .. } => {
                format!("tasks[key={task_key}].perspective")
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
            BlueprintError::MissingRepository { .. } => "blueprint/missing_repository".to_string(),
            BlueprintError::InvalidRepository { cause, .. } => cause.reason_key(),
            BlueprintError::SelfDependency { .. } => "blueprint/self_dependency".to_string(),
            BlueprintError::DuplicateDependency { .. } => {
                "blueprint/duplicate_dependency".to_string()
            }
            BlueprintError::PerspectiveOnNonReview { .. } => {
                "blueprint/perspective_on_non_review".to_string()
            }
            BlueprintError::UnknownPerspective { .. } => {
                "blueprint/unknown_perspective".to_string()
            }
        }
    }
}

/// Convert a Blueprint into a domain TaskTree.
///
/// Task IDs are built as `{workflow_id}:{key_path}` where key_path
/// is the `/`-separated path of task keys from root to the node.
///
/// Validates all constraints first (collecting all errors), then builds
/// the tree using the validated keys.
pub(super) fn to_task_tree(
    blueprint: &TaskTreeBlueprint,
    workflow_id: &WorkflowId,
    known_perspectives: &HashSet<String>,
) -> Result<TaskTree, Vec<BlueprintError>> {
    let validator = BlueprintValidator::new(known_perspectives);
    let validated = validator.validate(&blueprint.task)?;

    let root = &blueprint.task;
    let root_key = &validated.keys[root.key.as_str()];
    let root_id = TaskId::root(workflow_id, root_key);
    let mut nodes = HashMap::new();

    insert_node(
        &root_id,
        None,
        None,
        root,
        &mut nodes,
        &validated.keys,
        &validated.job_details,
    );
    collect_nodes(
        &root_id,
        root.plan_path.as_deref(),
        &root.children,
        &mut nodes,
        &validated.keys,
        &validated.job_details,
    );

    Ok(TaskTree::new(root_id, nodes))
}

fn insert_node(
    task_id: &TaskId,
    parent_id: Option<&TaskId>,
    parent_plan_path: Option<&str>,
    node: &TaskNode,
    nodes: &mut HashMap<TaskId, TaskTreeNode>,
    keys: &HashMap<&str, TaskKey>,
    job_details: &HashMap<&str, Option<JobDetail>>,
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

    let job_detail = job_details[node.key.as_str()].clone();

    nodes.insert(
        task_id.clone(),
        TaskTreeNode {
            id: task_id.clone(),
            parent_id: parent_id.cloned(),
            key,
            plan_path,
            priority: node.priority.map(Priority::from),
            children: child_ids,
            depends_on,
            job_detail,
        },
    );
}

fn collect_nodes(
    parent_task_id: &TaskId,
    parent_plan_path: Option<&str>,
    children: &[TaskNode],
    nodes: &mut HashMap<TaskId, TaskTreeNode>,
    keys: &HashMap<&str, TaskKey>,
    job_details: &HashMap<&str, Option<JobDetail>>,
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
            job_details,
        );

        if !child.children.is_empty() {
            collect_nodes(
                &child_task_id,
                child_plan_path,
                &child.children,
                nodes,
                keys,
                job_details,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use palette_domain::job::JobDetail;

    fn no_perspectives() -> HashSet<String> {
        HashSet::new()
    }

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
          plan_path: planning/api-plan/README.md
          repository:
            name: x7c1/palette-demo
            branch: main
          children:
            - key: api-plan-review
              type: review

    - key: execution
      depends_on: [planning]
      children:
        - key: api-impl
          type: craft
          plan_path: execution/api-impl/README.md
          repository:
            name: x7c1/palette-demo
            branch: main
          children:
            - key: api-impl-review
              type: review
"#;
        let blueprint: TaskTreeBlueprint = serde_yaml::from_str(yaml).unwrap();
        let tree = to_task_tree(&blueprint, &wf_id, &no_perspectives()).unwrap();

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
        assert!(planning.job_detail.is_none());
        assert_eq!(planning.children.len(), 1);
        assert!(planning.depends_on.is_empty());

        // api-plan (composite craft with review child)
        let api_plan = tree.find_by_key("api-plan").unwrap();
        assert!(matches!(api_plan.job_detail, Some(JobDetail::Craft { .. })));
        assert_eq!(
            api_plan.plan_path.as_deref(),
            Some("planning/api-plan/README.md")
        );
        assert!(api_plan.depends_on.is_empty());
        assert_eq!(api_plan.children.len(), 1);

        // api-plan-review (child of api-plan, review type, inherits plan_path)
        let review = tree.find_by_key("api-plan-review").unwrap();
        assert!(matches!(review.job_detail, Some(JobDetail::Review { .. })));
        assert_eq!(review.parent_id.as_ref().unwrap(), &api_plan.id);
        assert_eq!(
            review.plan_path.as_deref(),
            Some("planning/api-plan/README.md"),
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
        let errors = to_task_tree(&blueprint, &wf_id, &no_perspectives()).unwrap_err();
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
      repository:
        name: x7c1/palette-demo
        branch: main
"#;
        let blueprint: TaskTreeBlueprint = serde_yaml::from_str(yaml).unwrap();
        let errors = to_task_tree(&blueprint, &wf_id, &no_perspectives()).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], BlueprintError::MissingReviewChild { task_key } if task_key == "my-craft")
        );
    }

    #[test]
    fn rejects_craft_without_repository() {
        let wf_id = WorkflowId::parse("wf-test").unwrap();
        let yaml = r#"
task:
  key: root
  children:
    - key: my-craft
      type: craft
      children:
        - key: my-review
          type: review
"#;
        let blueprint: TaskTreeBlueprint = serde_yaml::from_str(yaml).unwrap();
        let errors = to_task_tree(&blueprint, &wf_id, &no_perspectives()).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], BlueprintError::MissingRepository { task_key } if task_key == "my-craft")
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
      repository:
        name: x7c1/palette-demo
        branch: main
"#;
        let blueprint: TaskTreeBlueprint = serde_yaml::from_str(yaml).unwrap();
        let errors = to_task_tree(&blueprint, &wf_id, &no_perspectives()).unwrap_err();
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn perspective_on_review_with_known_name() {
        let wf_id = WorkflowId::parse("wf-test").unwrap();
        let yaml = r#"
task:
  key: root
  children:
    - key: my-craft
      type: craft
      plan_path: plans/impl
      repository:
        name: x7c1/palette-demo
        branch: main
      children:
        - key: my-review
          type: review
          perspective: rust-review
"#;
        let perspectives: HashSet<String> = ["rust-review".to_string()].into();
        let blueprint: TaskTreeBlueprint = serde_yaml::from_str(yaml).unwrap();
        let tree = to_task_tree(&blueprint, &wf_id, &perspectives).unwrap();
        let review = tree.find_by_key("my-review").unwrap();
        assert!(matches!(
            &review.job_detail,
            Some(JobDetail::Review { perspective: Some(p) }) if p.as_ref() == "rust-review"
        ));
    }

    #[test]
    fn rejects_perspective_on_non_review_task() {
        let wf_id = WorkflowId::parse("wf-test").unwrap();
        let yaml = r#"
task:
  key: root
  children:
    - key: my-craft
      type: craft
      perspective: rust-review
      repository:
        name: x7c1/palette-demo
        branch: main
      children:
        - key: my-review
          type: review
"#;
        let perspectives: HashSet<String> = ["rust-review".to_string()].into();
        let blueprint: TaskTreeBlueprint = serde_yaml::from_str(yaml).unwrap();
        let errors = to_task_tree(&blueprint, &wf_id, &perspectives).unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, BlueprintError::PerspectiveOnNonReview { task_key } if task_key == "my-craft")));
    }

    #[test]
    fn rejects_unknown_perspective_name() {
        let wf_id = WorkflowId::parse("wf-test").unwrap();
        let yaml = r#"
task:
  key: root
  children:
    - key: my-craft
      type: craft
      plan_path: plans/impl
      repository:
        name: x7c1/palette-demo
        branch: main
      children:
        - key: my-review
          type: review
          perspective: nonexistent
"#;
        let blueprint: TaskTreeBlueprint = serde_yaml::from_str(yaml).unwrap();
        let errors = to_task_tree(&blueprint, &wf_id, &no_perspectives()).unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, BlueprintError::UnknownPerspective { perspective, .. } if perspective == "nonexistent")));
    }
}
