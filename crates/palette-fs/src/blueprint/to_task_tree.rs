use super::{TaskNode, TaskTreeBlueprint};
use palette_domain::job::{JobType, Priority, Repository};
use palette_domain::task::{TaskId, TaskTree, TaskTreeNode};
use std::collections::HashMap;

impl TaskTreeBlueprint {
    /// Convert this Blueprint into a domain TaskTree.
    pub fn to_task_tree(&self) -> TaskTree {
        let root = &self.task;
        let root_id = TaskId::new(&root.id);
        let mut nodes = HashMap::new();

        insert_node(&root.id, &root_id, None, root, &mut nodes);
        collect_nodes(&root.id, &root_id, &root.children, &mut nodes);

        TaskTree::new(root_id, nodes)
    }
}

fn insert_node(
    id_str: &str,
    task_id: &TaskId,
    parent_id: Option<&TaskId>,
    node: &TaskNode,
    nodes: &mut HashMap<TaskId, TaskTreeNode>,
) {
    let child_ids: Vec<TaskId> = node
        .children
        .iter()
        .map(|c| TaskId::new(format!("{id_str}/{}", c.id)))
        .collect();

    let depends_on: Vec<TaskId> = if let Some(parent) = parent_id {
        let parent_str = parent.as_ref();
        node.depends_on
            .iter()
            .map(|dep| TaskId::new(format!("{parent_str}/{dep}")))
            .collect()
    } else {
        vec![]
    };

    nodes.insert(
        task_id.clone(),
        TaskTreeNode {
            id: task_id.clone(),
            parent_id: parent_id.cloned(),
            title: node.title.clone(),
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
    parent_id_str: &str,
    parent_task_id: &TaskId,
    children: &[TaskNode],
    nodes: &mut HashMap<TaskId, TaskTreeNode>,
) {
    for child in children {
        let child_id_str = format!("{parent_id_str}/{}", child.id);
        let child_task_id = TaskId::new(&child_id_str);

        insert_node(
            &child_id_str,
            &child_task_id,
            Some(parent_task_id),
            child,
            nodes,
        );

        if !child.children.is_empty() {
            collect_nodes(&child_id_str, &child_task_id, &child.children, nodes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TaskTreeBlueprint;
    use super::*;

    #[test]
    fn builds_flat_index_from_nested_blueprint() {
        let yaml = r#"
task:
  id: 2026/feature-x
  title: Add feature X
  children:
    - id: planning
      title: Planning phase
      children:
        - id: api-plan
          title: API plan
          type: craft
          plan_path: 2026/feature-x/planning/api-plan
          children:
            - id: api-plan-review
              title: API plan review
              type: review

    - id: execution
      title: Execution phase
      depends_on: [planning]
      children:
        - id: api-impl
          title: API implementation
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
        assert_eq!(planning.title, "Planning phase");
        assert_eq!(
            planning.parent_id.as_ref().unwrap(),
            &TaskId::new("2026/feature-x")
        );
        assert!(planning.job_type.is_none());
        assert_eq!(planning.children.len(), 1);
        assert!(planning.depends_on.is_empty());

        // api-plan (composite craft with review child)
        let api_plan = tree
            .get(&TaskId::new("2026/feature-x/planning/api-plan"))
            .unwrap();
        assert_eq!(api_plan.job_type, Some(JobType::Craft));
        assert_eq!(
            api_plan.plan_path.as_deref(),
            Some("2026/feature-x/planning/api-plan")
        );
        assert!(api_plan.depends_on.is_empty());
        assert_eq!(api_plan.children.len(), 1);

        // api-plan-review (child of api-plan, review type)
        let review = tree
            .get(&TaskId::new(
                "2026/feature-x/planning/api-plan/api-plan-review",
            ))
            .unwrap();
        assert_eq!(review.job_type, Some(JobType::Review));
        assert_eq!(
            review.parent_id.as_ref().unwrap(),
            &TaskId::new("2026/feature-x/planning/api-plan")
        );
        assert!(review.depends_on.is_empty());

        // execution (depends on planning)
        let execution = tree.get(&TaskId::new("2026/feature-x/execution")).unwrap();
        assert_eq!(
            execution.depends_on,
            vec![TaskId::new("2026/feature-x/planning")]
        );

        // Total: root + planning + api-plan + api-plan/api-plan-review + execution + api-impl = 6
        assert_eq!(tree.task_ids().count(), 6);
    }
}
