use super::BlueprintDiff;
use palette_domain::task::{TaskId, TaskStatus, TaskTree};
use std::collections::HashMap;

/// A single validation error for a Blueprint change.
#[derive(Debug)]
pub struct ValidationError {
    pub task_id: String,
    pub message: String,
}

/// Result of validating a Blueprint diff against change rules.
#[derive(Debug)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Validate a Blueprint diff against the change rules.
///
/// Rules:
/// - Cannot add children to a Completed, InProgress, or Suspended task
/// - Cannot delete a Completed, InProgress, or Suspended task
/// - Cannot modify the subtree of a Completed, InProgress, or Suspended task
///   (even Pending/Ready children under an immutable ancestor are frozen)
pub fn validate_diff(
    diff: &BlueprintDiff,
    tree: &TaskTree,
    db_statuses: &HashMap<TaskId, TaskStatus>,
) -> ValidationResult {
    let mut errors = Vec::new();

    // Check added tasks: the nearest existing parent must not be immutable
    for task_id in &diff.added_tasks {
        if let Some(ancestor) = find_immutable_parent_in_tree(task_id, tree, db_statuses) {
            errors.push(ValidationError {
                task_id: task_id.to_string(),
                message: format!(
                    "cannot add task under {} ancestor {}",
                    db_statuses[&ancestor], ancestor
                ),
            });
        }
    }

    // Check removed tasks: task itself must not be immutable,
    // and its direct parent must not be immutable
    for task_id in &diff.removed_tasks {
        if let Some(&status) = db_statuses.get(task_id)
            && is_immutable(status)
        {
            errors.push(ValidationError {
                task_id: task_id.to_string(),
                message: format!("cannot delete {} task", status),
            });
            continue;
        }
        if let Some(ancestor) = find_immutable_parent_by_id(task_id, db_statuses) {
            errors.push(ValidationError {
                task_id: task_id.to_string(),
                message: format!(
                    "cannot delete task in {} subtree of {}",
                    db_statuses[&ancestor], ancestor
                ),
            });
        }
    }

    ValidationResult { errors }
}

fn is_immutable(status: TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Completed | TaskStatus::InProgress | TaskStatus::Suspended
    )
}

/// Walk up the tree to find the nearest ancestor that exists in the DB,
/// and check if it is immutable. This is the "direct parent" check:
/// a Pending parent forms a mutable boundary, so we stop there.
/// Used for added tasks that exist in the new Blueprint.
fn find_immutable_parent_in_tree(
    task_id: &TaskId,
    tree: &TaskTree,
    db_statuses: &HashMap<TaskId, TaskStatus>,
) -> Option<TaskId> {
    let node = tree.get(task_id)?;
    let mut current_id = node.parent_id.clone()?;

    loop {
        if let Some(&status) = db_statuses.get(&current_id) {
            // Found the nearest existing ancestor — check only this one
            return if is_immutable(status) {
                Some(current_id)
            } else {
                None
            };
        }
        // Parent is also new (not in DB), keep walking up
        match tree.get(&current_id).and_then(|n| n.parent_id.clone()) {
            Some(parent) => current_id = parent,
            None => return None,
        }
    }
}

/// Check if the direct parent (in the DB) of a removed task is immutable.
/// Used for removed tasks that are no longer in the Blueprint's tree.
fn find_immutable_parent_by_id(
    task_id: &TaskId,
    db_statuses: &HashMap<TaskId, TaskStatus>,
) -> Option<TaskId> {
    let mut current = task_id.parent();
    while let Some(parent_id) = current {
        if let Some(&status) = db_statuses.get(&parent_id) {
            // Found the nearest existing ancestor — check only this one
            return if is_immutable(status) {
                Some(parent_id)
            } else {
                None
            };
        }
        current = parent_id.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use palette_domain::task::{TaskKey, TaskTreeNode};

    fn wf() -> palette_domain::workflow::WorkflowId {
        palette_domain::workflow::WorkflowId::parse("wf-1").unwrap()
    }

    fn root_id() -> TaskId {
        TaskId::root(&wf(), &TaskKey::new("root"))
    }

    fn child_id(key: &str) -> TaskId {
        root_id().child(&TaskKey::new(key.to_string()))
    }

    fn make_tree_with_children(children: &[&str]) -> TaskTree {
        let rid = root_id();
        let mut nodes = HashMap::new();

        nodes.insert(
            rid.clone(),
            TaskTreeNode {
                id: rid.clone(),
                parent_id: None,
                key: TaskKey::new("root"),
                plan_path: None,
                job_type: None,
                priority: None,
                repository: None,
                children: children
                    .iter()
                    .map(|k| rid.child(&TaskKey::new(*k)))
                    .collect(),
                depends_on: vec![],
            },
        );

        for key in children {
            let cid = rid.child(&TaskKey::new(*key));
            nodes.insert(
                cid.clone(),
                TaskTreeNode {
                    id: cid.clone(),
                    parent_id: Some(rid.clone()),
                    key: TaskKey::new(*key),
                    plan_path: None,
                    job_type: None,
                    priority: None,
                    repository: None,
                    children: vec![],
                    depends_on: vec![],
                },
            );
        }

        TaskTree::new(rid, nodes)
    }

    #[test]
    fn allows_adding_under_pending_parent() {
        let tree = make_tree_with_children(&["a", "b"]);
        let db: HashMap<_, _> = [
            (root_id(), TaskStatus::Pending),
            (child_id("a"), TaskStatus::Pending),
        ]
        .into();
        let diff = BlueprintDiff {
            added_tasks: vec![child_id("b")],
            removed_tasks: vec![],
        };
        let result = validate_diff(&diff, &tree, &db);
        assert!(result.is_valid());
    }

    #[test]
    fn rejects_adding_under_completed_parent() {
        let tree = make_tree_with_children(&["a", "b"]);
        let db: HashMap<_, _> = [
            (root_id(), TaskStatus::Completed),
            (child_id("a"), TaskStatus::Completed),
        ]
        .into();
        let diff = BlueprintDiff {
            added_tasks: vec![child_id("b")],
            removed_tasks: vec![],
        };
        let result = validate_diff(&diff, &tree, &db);
        assert!(!result.is_valid());
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].message.contains("completed"));
    }

    #[test]
    fn rejects_adding_under_in_progress_parent() {
        let tree = make_tree_with_children(&["a", "b"]);
        let db: HashMap<_, _> = [
            (root_id(), TaskStatus::InProgress),
            (child_id("a"), TaskStatus::Pending),
        ]
        .into();
        let diff = BlueprintDiff {
            added_tasks: vec![child_id("b")],
            removed_tasks: vec![],
        };
        let result = validate_diff(&diff, &tree, &db);
        assert!(!result.is_valid());
    }

    #[test]
    fn rejects_deleting_completed_task() {
        let tree = make_tree_with_children(&["a"]);
        let db: HashMap<_, _> = [
            (root_id(), TaskStatus::InProgress),
            (child_id("a"), TaskStatus::Pending),
            (child_id("b"), TaskStatus::Completed),
        ]
        .into();
        let diff = BlueprintDiff {
            added_tasks: vec![],
            removed_tasks: vec![child_id("b")],
        };
        let result = validate_diff(&diff, &tree, &db);
        assert!(!result.is_valid());
        assert!(result.errors[0].message.contains("completed"));
    }

    #[test]
    fn allows_deleting_pending_task_under_pending_parent() {
        let tree = make_tree_with_children(&["a"]);
        let db: HashMap<_, _> = [
            (root_id(), TaskStatus::Pending),
            (child_id("a"), TaskStatus::Pending),
            (child_id("b"), TaskStatus::Pending),
        ]
        .into();
        let diff = BlueprintDiff {
            added_tasks: vec![],
            removed_tasks: vec![child_id("b")],
        };
        let result = validate_diff(&diff, &tree, &db);
        assert!(result.is_valid());
    }

    #[test]
    fn rejects_deleting_pending_task_under_in_progress_parent() {
        let tree = make_tree_with_children(&["a"]);
        let db: HashMap<_, _> = [
            (root_id(), TaskStatus::InProgress),
            (child_id("a"), TaskStatus::Pending),
            (child_id("b"), TaskStatus::Pending),
        ]
        .into();
        let diff = BlueprintDiff {
            added_tasks: vec![],
            removed_tasks: vec![child_id("b")],
        };
        let result = validate_diff(&diff, &tree, &db);
        assert!(!result.is_valid());
        assert!(result.errors[0].message.contains("in_progress"));
    }
}
