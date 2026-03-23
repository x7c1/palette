mod deliver_queued_messages;
mod deliver_to_all_idle;
mod handle_event;
mod process_effects;
mod resume_booting_watchers;
mod save_state;
mod spawn_member;
mod spawn_readiness_watcher;
mod spawn_supervisor;
mod start;

use palette_db::Database;
use palette_docker::DockerManager;
use palette_domain::server::PersistentState;
use std::sync::Arc;

use crate::DockerConfig;

pub struct Orchestrator {
    pub db: Arc<Database>,
    pub docker: DockerManager,
    pub docker_config: DockerConfig,
    pub plan_dir: String,
    pub tmux: Arc<palette_tmux::TmuxManager>,
    pub infra: Arc<tokio::sync::Mutex<PersistentState>>,
    pub state_path: String,
}
