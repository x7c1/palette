mod deliver_queued_messages;
mod deliver_to_all_idle;
mod handle_event;
mod process_effects;
mod resume_booting_watchers;
mod save_state;
mod spawn_member;
mod spawn_readiness_watcher;
mod start;

use palette_db::Database;
use palette_docker::DockerManager;
use palette_domain::agent::AgentId;
use palette_domain::server::PersistentState;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::DockerConfig;

pub struct Orchestrator {
    pub db: Arc<Database>,
    pub docker: DockerManager,
    pub docker_config: DockerConfig,
    pub plan_dir: String,
    pub tmux: Arc<palette_tmux::TmuxManager>,
    pub infra: Arc<tokio::sync::Mutex<PersistentState>>,
    pub state_path: String,
    member_counter: AtomicUsize,
}

impl Orchestrator {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: Arc<Database>,
        docker: DockerManager,
        docker_config: DockerConfig,
        plan_dir: String,
        tmux: Arc<palette_tmux::TmuxManager>,
        infra: Arc<tokio::sync::Mutex<PersistentState>>,
        state_path: String,
        initial_member_count: usize,
    ) -> Self {
        Self {
            db,
            docker,
            docker_config,
            plan_dir,
            tmux,
            infra,
            state_path,
            member_counter: AtomicUsize::new(initial_member_count),
        }
    }

    /// Generate the next member ID using a monotonic counter.
    /// IDs never repeat even after members are destroyed.
    pub(super) fn next_member_id(&self) -> AgentId {
        let seq = self.member_counter.fetch_add(1, Ordering::Relaxed);
        AgentId::next_member(seq)
    }
}
