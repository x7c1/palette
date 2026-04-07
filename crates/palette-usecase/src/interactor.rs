use crate::task_store::TaskStore;
use crate::{BlueprintReader, ContainerRuntime, DataStore, GitHubReviewPort, TerminalSession};
use palette_domain::workflow::WorkflowId;

/// Mediates all external resource access for the application.
///
/// Holds trait objects for container runtime, terminal session, data store,
/// blueprint reader, and GitHub review port. The orchestrator and server
/// access external resources exclusively through this struct.
pub struct Interactor {
    pub container: Box<dyn ContainerRuntime>,
    pub terminal: Box<dyn TerminalSession>,
    pub data_store: Box<dyn DataStore>,
    pub blueprint: Box<dyn BlueprintReader>,
    pub github_review: Box<dyn GitHubReviewPort>,
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
    ) -> Result<TaskStore<'_>, crate::TaskStoreError> {
        TaskStore::from_interactor(
            self.data_store.as_ref(),
            self.blueprint.as_ref(),
            workflow_id,
        )
    }
}
