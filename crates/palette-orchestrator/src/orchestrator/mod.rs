mod assign_deferred_jobs;
mod clean_orphan_containers;
mod deliver_queued_messages;
mod deliver_to_all_idle;
mod handle_event;
mod orchestrator_task;
mod process_effects;
mod recover_from_crash;
mod resume_booting_watchers;
mod shutdown;
mod spawn_member;
mod spawn_readiness_watcher;
mod spawn_supervisor;
mod start;
mod suspend;
mod validate_artifacts;
mod worker_monitor;
pub mod workspace;

use palette_domain::server::ServerEvent;
use palette_usecase::Interactor;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::DockerConfig;
use workspace::WorkspaceManager;

pub struct Orchestrator {
    pub interactor: Arc<Interactor>,
    pub docker_config: DockerConfig,
    pub plan_dir: String,
    pub session_name: String,
    pub cancel_token: CancellationToken,
    pub workspace_manager: WorkspaceManager,
    /// Sender for events back to the orchestrator event loop.
    /// Used by orchestrator tasks to report command completion.
    pub event_tx: mpsc::UnboundedSender<ServerEvent>,
}
