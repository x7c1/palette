use super::{TaskNode, TaskTreeBlueprint};
use palette_domain::job::{JobType, Priority, Repository};
use palette_domain::task::{TaskId, TaskTree, TaskTreeNode};
use palette_domain::workflow::WorkflowId;
use std::collections::HashMap;

impl TaskTreeBlueprint {
    /// Convert this Blueprint into a domain TaskTree.
    ///
    /// Task IDs are built as `{workflow_id}:{key_path}` where key_path
    /// is the `/`-separated path of keys from root to the node.
    pub fn to_task_tree(&self, workflow_id: &WorkflowId) -> TaskTree {
        let root = &self.task;
        let wf = workflow_id.as_ref();
        let root_id = TaskId::new(format!("{wf}:{}", root.key));
        let mut nodes = HashMap::new();

        insert_node(wf, &root.key, &root_id, None, root, &mut nodes);
        collect_nodes(wf, &root.key, &root_id, &root.children, &mut nodes);

        TaskTree::new(root_id, nodes)
    }
}

fn insert_node(
    wf: &str,
    key_path: &str,
    task_id: &TaskId,
    parent_id: Option<&TaskId>,
    node: &TaskNode,
    nodes: &mut HashMap<TaskId, TaskTreeNode>,
) {
    let child_ids: Vec<TaskId> = node
        .children
        .iter()
        .map(|c| TaskId::new(format!("{wf}:{key_path}/{}", c.key)))
        .collect();

    let depends_on: Vec<TaskId> = if let Some(parent) = parent_id {
        // depends_on references sibling keys; parent's key_path is the prefix
        let parent_key_path = &parent.as_ref()[wf.len() + 1..]; // skip "wf:" prefix
        node.depends_on
            .iter()
            .map(|dep| TaskId::new(format!("{wf}:{parent_key_path}/{dep}")))
            .collect()
    } else {
        vec![]
    };

    nodes.insert(
        task_id.clone(),
        TaskTreeNode {
            id: task_id.clone(),
            parent_id: parent_id.cloned(),
            key: node.key.clone(),
            plan_path: node.plan_path.clone(),
            job_type: node.job_type.map(JobType::from),
            description: node.description.clone(),
            priority: node.priority.map(Priority::from),
            repository: node.repository.clone().map(Repository::from),
            children: child_ids,
            depends_on,
        },
    );
}

fn collect_nodes(
    wf: &str,
    parent_key_path: &str,
    parent_task_id: &TaskId,
    children: &[TaskNode],
    nodes: &mut HashMap<TaskId, TaskTreeNode>,
) {
    for child in children {
        let child_key_path = format!("{parent_key_path}/{}", child.key);
        let child_task_id = TaskId::new(format!("{wf}:{child_key_path}"));

        insert_node(
            wf,
            &child_key_path,
            &child_task_id,
            Some(parent_task_id),
            child,
            nodes,
        );

        if !child.children.is_empty() {
            collect_nodes(wf, &child_key_path, &child_task_id, &child.children, nodes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TaskTreeBlueprint;
    use super::*;

    #[test]
    fn builds_flat_index_from_nested_blueprint() {
        let wf_id = WorkflowId::new("wf-test");
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
"#;
        let blueprint: TaskTreeBlueprint = serde_yaml::from_str(yaml).unwrap();
        let tree = blueprint.to_task_tree(&wf_id);

        // Root
        assert_eq!(tree.root_id(), &TaskId::new("wf-test:feature-x"));
        let root = tree.find_by_key("feature-x").unwrap();
        assert_eq!(root.key, "feature-x");
        assert!(root.parent_id.is_none());
        assert_eq!(root.children.len(), 2);

        // planning (composite, no job_type)
        let planning = tree.find_by_key("planning").unwrap();
        assert_eq!(planning.id, TaskId::new("wf-test:feature-x/planning"));
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

        // api-plan-review (child of api-plan, review type)
        let review = tree.find_by_key("api-plan-review").unwrap();
        assert_eq!(review.job_type, Some(JobType::Review));
        assert_eq!(review.parent_id.as_ref().unwrap(), &api_plan.id);
        assert!(review.depends_on.is_empty());

        // execution (depends on planning)
        let execution = tree.find_by_key("execution").unwrap();
        assert_eq!(execution.depends_on, vec![planning.id.clone()]);

        // Total: root + planning + api-plan + api-plan-review + execution + api-impl = 6
        assert_eq!(tree.task_ids().count(), 6);
    }
}
