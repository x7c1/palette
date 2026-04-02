use super::Orchestrator;
use super::PendingActions;
use palette_domain::task::{TaskId, TaskStatus};
use palette_domain::worker::WorkerRole;
use palette_domain::workflow::WorkflowId;
use palette_usecase::TaskRuleEngine;

impl Orchestrator {
    /// Handle ActivateWorkflow: set root to InProgress, spawn supervisor,
    /// and recursively activate ready children.
    pub(in crate::orchestrator) fn activate_workflow(
        &self,
        workflow_id: &WorkflowId,
    ) -> crate::Result<PendingActions> {
        let mut result = PendingActions::new();

        let task_store = self.interactor.create_task_store(workflow_id)?;
        let task_engine = TaskRuleEngine::new(&task_store);

        let root = task_store.get_task(task_store.root_id()).ok_or_else(|| {
            crate::Error::TaskNotFound {
                task_id: task_store.root_id().clone(),
            }
        })?;

        // Spawn Approver for root
        match self.handle_spawn_supervisor(&root.id, WorkerRole::Approver) {
            Ok(sup_id) => result.watch_only.push(sup_id),
            Err(e) => {
                tracing::error!(error = %e, task_id = %root.id, "failed to spawn root supervisor");
            }
        }

        // Root → InProgress
        task_store.update_task_status(&root.id, TaskStatus::InProgress)?;

        // Resolve children recursively
        let child_ids: Vec<TaskId> = root.children.iter().map(|c| c.id.clone()).collect();
        let ready_ids = task_engine.resolve_ready_tasks(&child_ids);
        for ready_id in &ready_ids {
            tracing::info!(task_id = %ready_id, status = ?TaskStatus::Ready, "task activated");
            result = result.merge(self.activate_ready_task(ready_id, &task_store, &task_engine)?);
        }

        Ok(result)
    }

    /// Handle ActivateNewTasks: find pending tasks in the workflow and activate
    /// any that are now ready.
    pub(in crate::orchestrator) fn activate_new_tasks(
        &self,
        workflow_id: &WorkflowId,
    ) -> crate::Result<PendingActions> {
        let mut result = PendingActions::new();

        let task_store = self.interactor.create_task_store(workflow_id)?;
        let task_engine = TaskRuleEngine::new(&task_store);

        let pending_ids: Vec<TaskId> = task_store
            .tree()
            .task_ids()
            .filter(|id| {
                task_store
                    .get_task(id)
                    .is_some_and(|t| t.status == TaskStatus::Pending)
            })
            .cloned()
            .collect();

        if pending_ids.is_empty() {
            return Ok(result);
        }

        let ready_ids = task_engine.resolve_ready_tasks(&pending_ids);
        for ready_id in &ready_ids {
            tracing::info!(task_id = %ready_id, status = ?TaskStatus::Ready, "task activated (blueprint apply)");
            result = result.merge(self.activate_ready_task(ready_id, &task_store, &task_engine)?);
        }

        Ok(result)
    }
}
