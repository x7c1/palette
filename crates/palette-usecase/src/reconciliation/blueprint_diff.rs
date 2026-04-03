use palette_domain::task::{TaskId, TaskStatus, TaskTree};
use std::collections::{HashMap, HashSet};

/// Diff between the current Blueprint and the DB state.
pub struct BlueprintDiff {
    /// Tasks present in the Blueprint but not registered in the DB.
    pub added_tasks: Vec<TaskId>,
    /// Tasks registered in the DB but absent from the Blueprint.
    pub removed_tasks: Vec<TaskId>,
}

/// Compare the Blueprint's task tree against DB task statuses to find added and removed tasks.
pub fn compute_diff(tree: &TaskTree, db_statuses: &HashMap<TaskId, TaskStatus>) -> BlueprintDiff {
    let blueprint_ids: HashSet<&TaskId> = tree.task_ids().collect();
    let db_ids: HashSet<&TaskId> = db_statuses.keys().collect();

    let added_tasks: Vec<TaskId> = blueprint_ids
        .difference(&db_ids)
        .map(|id| (*id).clone())
        .collect();

    let removed_tasks: Vec<TaskId> = db_ids
        .difference(&blueprint_ids)
        .map(|id| (*id).clone())
        .collect();

    BlueprintDiff {
        added_tasks,
        removed_tasks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use palette_domain::task::{TaskKey, TaskTree, TaskTreeNode};
    use palette_domain::workflow::WorkflowId;

    fn make_tree(workflow_id: &WorkflowId, keys: &[&str]) -> TaskTree {
        let root_key = TaskKey::parse("root").unwrap();
        let root_id = TaskId::root(workflow_id, &root_key);
        let mut nodes = HashMap::new();

        nodes.insert(
            root_id.clone(),
            TaskTreeNode {
                id: root_id.clone(),
                parent_id: None,
                key: root_key,
                plan_path: None,
                job_type: None,
                priority: None,
                repository: None,
                command: None,
                children: keys
                    .iter()
                    .map(|k| root_id.child(&TaskKey::parse(*k).unwrap()))
                    .collect(),
                depends_on: vec![],
            },
        );

        for key in keys {
            let child_id = root_id.child(&TaskKey::parse(*key).unwrap());
            nodes.insert(
                child_id.clone(),
                TaskTreeNode {
                    id: child_id.clone(),
                    parent_id: Some(root_id.clone()),
                    key: TaskKey::parse(*key).unwrap(),
                    plan_path: None,
                    job_type: None,
                    priority: None,
                    repository: None,
                    command: None,
                    children: vec![],
                    depends_on: vec![],
                },
            );
        }

        TaskTree::new(root_id, nodes)
    }

    #[test]
    fn no_changes() {
        let wf = WorkflowId::parse("wf-1").unwrap();
        let tree = make_tree(&wf, &["a", "b"]);
        let root_id = TaskId::root(&wf, &TaskKey::parse("root").unwrap());
        let db: HashMap<_, _> = [
            (root_id, TaskStatus::InProgress),
            (
                TaskId::root(&wf, &TaskKey::parse("root").unwrap())
                    .child(&TaskKey::parse("a").unwrap()),
                TaskStatus::Pending,
            ),
            (
                TaskId::root(&wf, &TaskKey::parse("root").unwrap())
                    .child(&TaskKey::parse("b").unwrap()),
                TaskStatus::Pending,
            ),
        ]
        .into();

        let diff = compute_diff(&tree, &db);
        assert!(diff.added_tasks.is_empty());
        assert!(diff.removed_tasks.is_empty());
    }

    #[test]
    fn detects_added_tasks() {
        let wf = WorkflowId::parse("wf-1").unwrap();
        let tree = make_tree(&wf, &["a", "b", "c"]);
        let root_id = TaskId::root(&wf, &TaskKey::parse("root").unwrap());
        let db: HashMap<_, _> = [
            (root_id, TaskStatus::InProgress),
            (
                TaskId::root(&wf, &TaskKey::parse("root").unwrap())
                    .child(&TaskKey::parse("a").unwrap()),
                TaskStatus::Pending,
            ),
            (
                TaskId::root(&wf, &TaskKey::parse("root").unwrap())
                    .child(&TaskKey::parse("b").unwrap()),
                TaskStatus::Pending,
            ),
        ]
        .into();

        let diff = compute_diff(&tree, &db);
        assert_eq!(diff.added_tasks.len(), 1);
        assert!(diff.added_tasks[0].as_ref().ends_with("/c"));
        assert!(diff.removed_tasks.is_empty());
    }

    #[test]
    fn detects_removed_tasks() {
        let wf = WorkflowId::parse("wf-1").unwrap();
        let tree = make_tree(&wf, &["a"]);
        let root_id = TaskId::root(&wf, &TaskKey::parse("root").unwrap());
        let db: HashMap<_, _> = [
            (root_id, TaskStatus::InProgress),
            (
                TaskId::root(&wf, &TaskKey::parse("root").unwrap())
                    .child(&TaskKey::parse("a").unwrap()),
                TaskStatus::Pending,
            ),
            (
                TaskId::root(&wf, &TaskKey::parse("root").unwrap())
                    .child(&TaskKey::parse("b").unwrap()),
                TaskStatus::Pending,
            ),
        ]
        .into();

        let diff = compute_diff(&tree, &db);
        assert!(diff.added_tasks.is_empty());
        assert_eq!(diff.removed_tasks.len(), 1);
        assert!(diff.removed_tasks[0].as_ref().ends_with("/b"));
    }
}
