mod clean_orphan_containers;
mod deliver_queued_messages;
mod deliver_to_all_idle;
mod handle_event;
mod process_effects;
mod resume_booting_watchers;
mod shutdown;
mod spawn_member;
mod spawn_readiness_watcher;
mod spawn_supervisor;
mod start;
mod worker_monitor;

use palette_db::Database;
use palette_docker::DockerManager;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use crate::DockerConfig;

pub struct Orchestrator {
    pub db: Arc<Database>,
    pub docker: DockerManager,
    pub docker_config: DockerConfig,
    pub plan_dir: String,
    pub tmux: Arc<palette_tmux::TmuxManager>,
    pub session_name: String,
    pub cancel_token: CancellationToken,
}
