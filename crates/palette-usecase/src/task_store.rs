use crate::{BlueprintReader, DataStore};
use palette_domain::task::{Task, TaskId, TaskStatus, TaskTree, TaskTreeNode};
use palette_domain::workflow::WorkflowId;
use std::cell::RefCell;
use std::collections::HashMap;

/// TaskStore implementation that combines a TaskTree (structure from Blueprint)
/// with task statuses (execution state from DataStore) to produce full Task objects.
///
/// Reads are served from in-memory data (TaskTree + cached statuses).
/// Writes go to both the in-memory cache and the data store.
pub struct TaskStore<'a> {
    data_store: &'a dyn DataStore,
    tree: TaskTree,
    workflow_id: WorkflowId,
    statuses: RefCell<HashMap<TaskId, TaskStatus>>,
}

impl<'a> TaskStore<'a> {
    /// Build a TaskStore from a TaskTree and a map of task statuses.
    pub fn new(
        data_store: &'a dyn DataStore,
        tree: TaskTree,
        workflow_id: WorkflowId,
        statuses: HashMap<TaskId, TaskStatus>,
    ) -> Self {
        Self {
            data_store,
            tree,
            workflow_id,
            statuses: RefCell::new(statuses),
        }
    }

    /// Build a TaskStore by reading the Blueprint file and task statuses from the data store.
    pub fn from_interactor(
        data_store: &'a dyn DataStore,
        blueprint: &dyn BlueprintReader,
        workflow_id: &WorkflowId,
    ) -> Result<Self, crate::TaskStoreError> {
        let workflow = data_store
            .get_workflow(workflow_id)
            .map_err(crate::TaskStoreError::DataStore)?
            .ok_or_else(|| crate::TaskStoreError::WorkflowNotFound {
                workflow_id: workflow_id.clone(),
            })?;

        let tree = blueprint
            .read_blueprint(std::path::Path::new(&workflow.blueprint_path), workflow_id)?;
        let statuses = data_store
            .get_task_statuses(workflow_id)
            .map_err(crate::TaskStoreError::DataStore)?;

        Ok(Self::new(data_store, tree, workflow_id.clone(), statuses))
    }

    pub fn tree(&self) -> &TaskTree {
        &self.tree
    }

    pub fn root_id(&self) -> &TaskId {
        self.tree.root_id()
    }

    fn build_task(&self, node: &TaskTreeNode) -> Task {
        let statuses = self.statuses.borrow();
        let status = statuses
            .get(&node.id)
            .copied()
            .unwrap_or(TaskStatus::Pending);

        let children: Vec<Task> = node
            .children
            .iter()
            .filter_map(|child_id| self.tree.get(child_id))
            .map(|child_node| self.build_task(child_node))
            .collect();

        Task {
            id: node.id.clone(),
            workflow_id: self.workflow_id.clone(),
            parent_id: node.parent_id.clone(),
            key: node.key.clone(),
            plan_path: node.plan_path.clone(),
            job_type: node.job_type,
            priority: node.priority,
            repository: node.repository.clone(),
            command: node.command.clone(),
            status,
            children,
            depends_on: node.depends_on.clone(),
        }
    }
}

impl TaskStore<'_> {
    pub fn get_task(&self, id: &TaskId) -> Option<Task> {
        self.tree.get(id).map(|node| self.build_task(node))
    }

    pub fn get_child_tasks(&self, parent_id: &TaskId) -> Vec<Task> {
        let Some(parent_node) = self.tree.get(parent_id) else {
            return vec![];
        };
        parent_node
            .children
            .iter()
            .filter_map(|child_id| self.tree.get(child_id))
            .map(|child_node| self.build_task(child_node))
            .collect()
    }

    pub fn update_task_status(
        &self,
        id: &TaskId,
        status: TaskStatus,
    ) -> Result<(), crate::TaskStoreError> {
        self.data_store
            .update_task_status(id, status)
            .map_err(crate::TaskStoreError::DataStore)?;
        self.statuses.borrow_mut().insert(id.clone(), status);
        Ok(())
    }
}
