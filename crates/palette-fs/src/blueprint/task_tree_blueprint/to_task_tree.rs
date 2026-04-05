use super::{TaskNode, TaskTreeBlueprint};
use palette_domain::job::{InvalidRepository, JobDetail, JobType, Priority, Repository};
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
    /// Task depends on itself.
    SelfDependency { task_key: String },
    /// Same dependency listed more than once.
    DuplicateDependency { task_key: String, dep: String },
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
            BlueprintError::SelfDependency { task_key } => {
                format!("tasks[key={task_key}].depends_on")
            }
            BlueprintError::DuplicateDependency { task_key, dep } => {
                format!("tasks[key={task_key}].depends_on[{dep}]")
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
            BlueprintError::SelfDependency { .. } => "blueprint/self_dependency".to_string(),
            BlueprintError::DuplicateDependency { .. } => {
                "blueprint/duplicate_dependency".to_string()
            }
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
    let (key_errors, keys) = collect_keys(node);
    let structure_errors = check_craft_has_review(node);
    let repo_errors = check_repository(node);
    let dep_errors = validate_depends_on(node);

    let (child_errors, child_keys) = node.children.iter().map(validate_node).fold(
        (Vec::new(), HashMap::new()),
        |(mut errs, mut keys), (ce, ck)| {
            errs.extend(ce);
            keys.extend(ck);
            (errs, keys)
        },
    );

    let errors = key_errors
        .into_iter()
        .chain(structure_errors)
        .chain(repo_errors)
        .chain(dep_errors)
        .chain(child_errors)
        .collect();

    let mut all_keys = keys;
    all_keys.extend(child_keys);
    (errors, all_keys)
}

/// Parse the node's own key and depends_on keys.
/// Returns (errors, parsed_keys).
fn collect_keys(node: &TaskNode) -> (Vec<BlueprintError>, HashMap<&str, TaskKey>) {
    std::iter::once(node.key.as_str())
        .chain(node.depends_on.iter().map(String::as_str))
        .fold(
            (Vec::new(), HashMap::new()),
            |(mut errors, mut keys), raw| {
                match TaskKey::parse(raw) {
                    Ok(k) => {
                        keys.insert(raw, k);
                    }
                    Err(e) => errors.push(BlueprintError::InvalidKey(e)),
                }
                (errors, keys)
            },
        )
}

/// Check that craft tasks have at least one review child.
fn check_craft_has_review(node: &TaskNode) -> Option<BlueprintError> {
    let job_type = node.job_type?;
    if !matches!(JobType::from(job_type), JobType::Craft) {
        return None;
    }
    let has_review = node.children.iter().any(|c| {
        c.job_type.is_some_and(|jt| {
            matches!(
                JobType::from(jt),
                JobType::Review | JobType::ReviewIntegrate
            )
        })
    });
    if has_review {
        None
    } else {
        Some(BlueprintError::MissingReviewChild {
            task_key: node.key.clone(),
        })
    }
}

/// Check that the repository (if present) has valid name and branch.
fn check_repository(node: &TaskNode) -> Option<BlueprintError> {
    let repo = node.repository.as_ref()?;
    Repository::parse(&repo.name, &repo.branch)
        .err()
        .map(|cause| BlueprintError::InvalidRepository {
            task_key: node.key.clone(),
            cause,
        })
}

/// Check depends_on for self-dependency and duplicates.
fn validate_depends_on(node: &TaskNode) -> Vec<BlueprintError> {
    use std::collections::HashSet;

    node.depends_on
        .iter()
        .fold(
            (Vec::new(), HashSet::new()),
            |(mut errors, mut seen), dep| {
                if dep == &node.key {
                    errors.push(BlueprintError::SelfDependency {
                        task_key: node.key.clone(),
                    });
                } else if !seen.insert(dep.as_str()) {
                    errors.push(BlueprintError::DuplicateDependency {
                        task_key: node.key.clone(),
                        dep: dep.clone(),
                    });
                }
                (errors, seen)
            },
        )
        .0
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

    let job_detail = node.job_type.map(JobType::from).map(|jt| match jt {
        JobType::Craft => {
            let repository = node
                .repository
                .clone()
                .and_then(|r| r.parse().ok())
                .expect("craft task must have a valid repository");
            JobDetail::Craft { repository }
        }
        JobType::Review => JobDetail::Review,
        JobType::ReviewIntegrate => JobDetail::ReviewIntegrate,
        JobType::Orchestrator => JobDetail::Orchestrator {
            command: node.command.clone(),
        },
        JobType::Operator => JobDetail::Operator,
    });

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
    use palette_domain::job::JobDetail;

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
            name: x7c1/palette
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
            name: x7c1/palette
            branch: main
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
        assert!(matches!(review.job_detail, Some(JobDetail::Review)));
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
      repository:
        name: x7c1/palette
        branch: main
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
      repository:
        name: x7c1/palette
        branch: main
"#;
        let blueprint: TaskTreeBlueprint = serde_yaml::from_str(yaml).unwrap();
        let errors = blueprint.to_task_tree(&wf_id).unwrap_err();
        assert_eq!(errors.len(), 2);
    }
}
