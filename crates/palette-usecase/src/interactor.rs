use crate::task_store::TaskStoreImpl;
use crate::{BlueprintReader, ContainerRuntime, DataStore, TerminalSession};
use palette_domain::workflow::WorkflowId;
use std::sync::Arc;

/// Mediates all external resource access for the application.
///
/// Holds trait objects for container runtime, terminal session, data store,
/// and blueprint reader. The orchestrator and server access external
/// resources exclusively through this struct.
pub struct Interactor {
    pub container: Arc<dyn ContainerRuntime>,
    pub terminal: Arc<dyn TerminalSession>,
    pub data_store: Arc<dyn DataStore>,
    pub blueprint: Arc<dyn BlueprintReader>,
}

impl Interactor {
    /// Create a workflow-scoped TaskStore for use with RuleEngine.
    ///
    /// The returned TaskStore is a short-lived object that caches task statuses
    /// at construction time. It should be used within a single request/operation
    /// and then discarded.
    pub fn create_task_store(
        &self,
        workflow_id: &WorkflowId,
    ) -> Result<TaskStoreImpl, Box<dyn std::error::Error + Send + Sync>> {
        TaskStoreImpl::from_interactor(
            Arc::clone(&self.data_store),
            self.blueprint.as_ref(),
            workflow_id,
        )
    }
}
