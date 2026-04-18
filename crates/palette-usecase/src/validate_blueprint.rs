use crate::blueprint_reader::{BlueprintReader, ReadBlueprintError};
use palette_domain::job::JobDetail;
use palette_domain::task::TaskTree;
use palette_domain::workflow::WorkflowId;
use std::path::Path;

/// Validate a Blueprint file and return the resulting `TaskTree` on success.
///
/// This is a side-effect-free read: no database writes, no network I/O,
/// no activation events. The caller decides how to handle each error
/// variant (validate endpoint returns 404 for `NotFound`; start endpoint
/// maps it into a `BlueprintInvalid` 400).
pub fn validate_blueprint(
    reader: &dyn BlueprintReader,
    path: &Path,
    workflow_id: &WorkflowId,
) -> Result<TaskTree, ReadBlueprintError> {
    reader.read_blueprint(path, workflow_id)
}

/// Summary of a validated Blueprint, used as the `summary` payload on
/// successful `POST /blueprints/validate` responses.
#[derive(Debug, Clone)]
pub struct BlueprintSummary {
    pub root_task_key: String,
    pub task_count: usize,
    pub craft_count: usize,
    pub review_count: usize,
    pub referenced_plans: Vec<String>,
}

impl BlueprintSummary {
    pub fn from_tree(tree: &TaskTree) -> Self {
        let root_task_key = tree
            .get(tree.root_id())
            .map(|n| n.key.as_ref().to_string())
            .unwrap_or_default();

        let mut craft_count = 0;
        let mut review_count = 0;
        let mut task_count = 0;
        let mut plans: Vec<String> = Vec::new();

        for id in tree.task_ids() {
            task_count += 1;
            let Some(node) = tree.get(id) else { continue };
            match &node.job_detail {
                Some(JobDetail::Craft { .. }) => craft_count += 1,
                Some(JobDetail::Review { .. }) => review_count += 1,
                _ => {}
            }
            if let Some(own) = node.plan_path.as_ref() {
                // Only count plans explicitly declared on this node,
                // not those inherited from an ancestor.
                let inherited_from_parent = node
                    .parent_id
                    .as_ref()
                    .and_then(|pid| tree.get(pid))
                    .and_then(|p| p.plan_path.as_deref())
                    == Some(own.as_str());
                if !inherited_from_parent && !plans.contains(own) {
                    plans.push(own.clone());
                }
            }
        }

        plans.sort();

        BlueprintSummary {
            root_task_key,
            task_count,
            craft_count,
            review_count,
            referenced_plans: plans,
        }
    }
}
