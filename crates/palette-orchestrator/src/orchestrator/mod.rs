pub(crate) mod handler;
pub(crate) mod infra;
mod lifecycle;

use palette_domain::job::JobDetail;
use palette_domain::server::ServerEvent;
use palette_domain::workflow::WorkflowId;
use palette_usecase::Interactor;
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
    /// Sender for events back to the orchestrator event loop.
    /// Used by orchestrator tasks to report command completion.
    pub event_tx: mpsc::UnboundedSender<ServerEvent>,
}

impl Orchestrator {
    /// Resolve the [`PlanLocation`] for a job in the given workflow.
    /// Looks up the workflow's blueprint path and consults the job's detail to
    /// decide whether the Blueprint is reachable via the workspace mount or
    /// must be mounted separately.
    pub(crate) fn resolve_plan_location(
        &self,
        workflow_id: &WorkflowId,
        job_detail: &JobDetail,
    ) -> crate::Result<PlanLocation> {
        let workflow = self
            .interactor
            .data_store
            .get_workflow(workflow_id)
            .map_err(crate::Error::External)?
            .ok_or_else(|| crate::Error::WorkflowNotFound {
                workflow_id: workflow_id.clone(),
            })?;
        Ok(plan_location::resolve(
            std::path::Path::new(&workflow.blueprint_path),
            job_detail,
        ))
    }
}
