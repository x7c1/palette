use crate::{BlueprintReader, DataStore};
use palette_domain::task::{Task, TaskId, TaskStatus, TaskStore, TaskTree, TaskTreeNode};
use palette_domain::workflow::WorkflowId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

/// TaskStore implementation that combines a TaskTree (structure from Blueprint)
/// with task statuses (execution state from DataStore) to produce full Task objects.
///
/// Reads are served from in-memory data (TaskTree + cached statuses).
/// Writes go to both the in-memory cache and the data store.
pub struct TaskStoreImpl {
    data_store: Arc<dyn DataStore>,
    tree: TaskTree,
    workflow_id: WorkflowId,
    statuses: RefCell<HashMap<TaskId, TaskStatus>>,
}

impl TaskStoreImpl {
    /// Build a TaskStoreImpl from a TaskTree and a map of task statuses.
    pub fn new(
        data_store: Arc<dyn DataStore>,
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

    /// Build a TaskStoreImpl by reading the Blueprint file and task statuses from the data store.
    pub fn from_interactor(
        data_store: Arc<dyn DataStore>,
        blueprint: &dyn BlueprintReader,
        workflow_id: &WorkflowId,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let workflow = data_store
            .get_workflow(workflow_id)?
            .ok_or_else(|| format!("workflow not found: {workflow_id}"))?;

        let tree = blueprint
            .read_blueprint(std::path::Path::new(&workflow.blueprint_path), workflow_id)?;
        let statuses = data_store.get_task_statuses(workflow_id)?;

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
            status,
            children,
            depends_on: node.depends_on.clone(),
        }
    }
}

impl TaskStore for TaskStoreImpl {
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn get_task(&self, id: &TaskId) -> Result<Option<Task>, Self::Error> {
        Ok(self.tree.get(id).map(|node| self.build_task(node)))
    }

    fn get_child_tasks(&self, parent_id: &TaskId) -> Result<Vec<Task>, Self::Error> {
        let Some(parent_node) = self.tree.get(parent_id) else {
            return Ok(vec![]);
        };
        Ok(parent_node
            .children
            .iter()
            .filter_map(|child_id| self.tree.get(child_id))
            .map(|child_node| self.build_task(child_node))
            .collect())
    }

    fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> Result<(), Self::Error> {
        self.data_store.update_task_status(id, status)?;
        self.statuses.borrow_mut().insert(id.clone(), status);
        Ok(())
    }
}
