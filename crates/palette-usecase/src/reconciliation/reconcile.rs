use super::BlueprintDiff;
use crate::data_store::{CreateTaskRequest, DataStore};
use palette_domain::task::{TaskId, TaskStatus, TaskTree};
use palette_domain::workflow::WorkflowId;
use std::collections::HashMap;

/// Result of executing a reconciliation.
#[derive(Debug)]
pub struct ReconciliationResult {
    pub tasks_created: usize,
    pub tasks_deleted: usize,
    /// Task IDs that transitioned from Ready back to Pending
    /// because their dependencies changed.
    pub tasks_demoted: Vec<TaskId>,
}

/// Execute reconciliation: register new tasks, delete removed tasks/jobs,
/// and re-evaluate Ready ↔ Pending transitions.
pub fn reconcile(
    data_store: &dyn DataStore,
    diff: &BlueprintDiff,
    workflow_id: &WorkflowId,
    tree: &TaskTree,
    db_statuses: &HashMap<TaskId, TaskStatus>,
) -> Result<ReconciliationResult, Box<dyn std::error::Error + Send + Sync>> {
    let mut tasks_created = 0;
    let mut tasks_deleted = 0;

    // 1. Register new tasks as Pending
    for task_id in &diff.added_tasks {
        data_store.create_task(&CreateTaskRequest {
            id: task_id.clone(),
            workflow_id: workflow_id.clone(),
        })?;
        tasks_created += 1;
    }

    // 2. Delete removed tasks (and their jobs)
    for task_id in &diff.removed_tasks {
        data_store.delete_jobs_by_task_id(task_id)?;
        data_store.delete_task(task_id)?;
        tasks_deleted += 1;
    }

    // 3. Re-evaluate Ready → Pending for tasks whose dependencies may have changed.
    //    A Ready task should go back to Pending if its dependencies (in the new
    //    Blueprint) are not all Completed.
    let tasks_demoted = demote_invalid_ready_tasks(data_store, tree, db_statuses)?;

    Ok(ReconciliationResult {
        tasks_created,
        tasks_deleted,
        tasks_demoted,
    })
}

/// Check all Ready tasks: if their dependencies (per the new Blueprint) are not
/// all Completed, demote them back to Pending.
fn demote_invalid_ready_tasks(
    data_store: &dyn DataStore,
    tree: &TaskTree,
    db_statuses: &HashMap<TaskId, TaskStatus>,
) -> Result<Vec<TaskId>, Box<dyn std::error::Error + Send + Sync>> {
    let mut demoted = Vec::new();

    for (task_id, &status) in db_statuses {
        if status != TaskStatus::Ready {
            continue;
        }
        let Some(node) = tree.get(task_id) else {
            continue;
        };
        let all_deps_done = node.depends_on.iter().all(|dep_id| {
            db_statuses
                .get(dep_id)
                .is_some_and(|&s| s == TaskStatus::Completed)
        });
        if !all_deps_done {
            data_store.update_task_status(task_id, TaskStatus::Pending)?;
            demoted.push(task_id.clone());
        }
    }

    Ok(demoted)
}
