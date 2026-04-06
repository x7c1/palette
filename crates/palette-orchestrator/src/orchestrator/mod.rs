pub(crate) mod handler;
pub(crate) mod infra;
mod lifecycle;

use palette_domain::server::ServerEvent;
use palette_usecase::Interactor;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::DockerConfig;
use crate::perspectives_config::ValidatedPerspectives;
use infra::workspace::WorkspaceManager;

pub struct Orchestrator {
    pub interactor: Arc<Interactor>,
    pub docker_config: DockerConfig,
    pub plan_dir: String,
    pub session_name: String,
    pub cancel_token: CancellationToken,
    pub workspace_manager: WorkspaceManager,
    pub perspectives: ValidatedPerspectives,
    /// Sender for events back to the orchestrator event loop.
    /// Used by orchestrator tasks to report command completion.
    pub event_tx: mpsc::UnboundedSender<ServerEvent>,
}
