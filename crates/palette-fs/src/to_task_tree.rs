use crate::blueprint::{TaskNode, TaskTreeBlueprint};
use palette_domain::job::JobType;
use palette_domain::task::{TaskId, TaskTree, TaskTreeNode};
use std::collections::HashMap;

impl TaskTreeBlueprint {
    /// Convert this Blueprint into a domain TaskTree.
    pub fn to_task_tree(&self) -> TaskTree {
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
    parent_task_id: &TaskId,
    children: &[TaskNode],
    nodes: &mut HashMap<TaskId, TaskTreeNode>,
) {
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
    use crate::blueprint::TaskTreeBlueprint;

    #[test]
    fn builds_flat_index_from_nested_blueprint() {
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
