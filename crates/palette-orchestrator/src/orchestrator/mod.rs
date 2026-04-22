pub(crate) mod handler;
pub(crate) mod infra;
mod lifecycle;

use palette_domain::server::ServerEvent;
use palette_domain::workflow::WorkflowId;
use palette_usecase::Interactor;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::DockerConfig;
use crate::perspectives_config::ValidatedPerspectives;
use infra::plan_location::{self, PlanLocation};
use infra::workspace::WorkspaceManager;

pub struct Orchestrator {
    pub interactor: Arc<Interactor>,
    pub docker_config: DockerConfig,
    pub session_name: String,
    pub cancel_token: CancellationToken,
    pub workspace_manager: WorkspaceManager,
    pub perspectives: ValidatedPerspectives,
    /// Maximum number of review rounds before an Escalation is raised.
    pub max_review_rounds: u32,
    /// Sender for events back to the orchestrator event loop.
    /// Used by orchestrator tasks to report command completion.
    pub event_tx: mpsc::UnboundedSender<ServerEvent>,
}

impl Orchestrator {
    /// Resolve the [`PlanLocation`] for a job in the given workflow.
    ///
    /// When `workspace_host_path` is `Some`, the resolver checks whether the
    /// Blueprint sits inside that workspace and returns
    /// [`PlanLocation::InsideWorkspace`] if so. Otherwise the Blueprint is
    /// treated as external and the caller must attach a separate plan mount.
    pub(crate) fn resolve_plan_location(
        &self,
        workflow_id: &WorkflowId,
        workspace_host_path: Option<&Path>,
    ) -> crate::Result<PlanLocation> {
        let workflow = self
            .interactor
            .data_store
            .require_workflow(workflow_id)
            .map_err(crate::Error::External)?;
        plan_location::resolve(
            std::path::Path::new(&workflow.blueprint_path),
            workspace_host_path,
        )
        .map_err(|e| crate::Error::External(Box::new(e)))
    }

    /// Look up the absolute host path of a workflow's Blueprint file.
    ///
    /// Used by [`infra::workspace::WorkspaceManager::create_workspace`] to
    /// decide whether the Blueprint should be imported into the workspace
    /// (Repo-inside-Plan mode).
    pub(crate) fn workflow_blueprint_path(
        &self,
        workflow_id: &WorkflowId,
    ) -> crate::Result<std::path::PathBuf> {
        let workflow = self
            .interactor
            .data_store
            .require_workflow(workflow_id)
            .map_err(crate::Error::External)?;
        Ok(std::path::PathBuf::from(workflow.blueprint_path))
    }
}
