use palette_db::Database;
use palette_docker::DockerManager;
use palette_domain::{AgentId, AgentStatus, PersistentState, RuleEngine, ServerEvent};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::{DockerConfig, deliver_queued_messages, process_effects};

/// Interval between readiness polls.
const READINESS_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3);

/// Maximum time to wait for Claude Code readiness.
const READINESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

pub struct Orchestrator {
    pub db: Arc<Database>,
    pub docker: DockerManager,
    pub docker_config: DockerConfig,
    pub tmux: Arc<palette_tmux::TmuxManager>,
    pub infra: Arc<tokio::sync::Mutex<PersistentState>>,
    pub state_path: String,
    pub rules: RuleEngine,
}

impl Orchestrator {
    /// Start the event processing loop.
    pub fn start(self: Arc<Self>, mut rx: mpsc::UnboundedReceiver<ServerEvent>) {
        let this = self;
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                Self::handle_event(&this, event).await;
            }
        });
    }

    /// Start readiness watchers for any agents currently in Booting state.
    pub fn resume_booting_watchers(this: &Arc<Self>, infra: &PersistentState) {
        for leader in &infra.leaders {
            if leader.status == AgentStatus::Booting {
                Self::spawn_readiness_watcher(this, leader.id.clone());
            }
        }
        for member in &infra.members {
            if member.status == AgentStatus::Booting {
                Self::spawn_readiness_watcher(this, member.id.clone());
            }
        }
    }

    async fn handle_event(this: &Arc<Self>, event: ServerEvent) {
        match event {
            ServerEvent::ProcessEffects { effects } => {
                let mut infra = this.infra.lock().await;
                match process_effects(
                    &effects,
                    &this.db,
                    &mut infra,
                    &this.docker,
                    &*this.tmux,
                    &this.docker_config,
                ) {
                    Ok(deliveries) => {
                        for d in &deliveries {
                            let _ = deliver_queued_messages(
                                &d.target_id,
                                &this.db,
                                &mut infra,
                                &*this.tmux,
                            );
                        }
                        Self::save_state(this, &infra);
                        drop(infra);
                        for d in deliveries {
                            Self::spawn_readiness_watcher(this, d.target_id);
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "failed to process effects");
                    }
                }
            }
            ServerEvent::DeliverMessages { target_id } => {
                let mut infra = this.infra.lock().await;
                let _ = deliver_queued_messages(&target_id, &this.db, &mut infra, &*this.tmux);
            }
            ServerEvent::NotifyDeliveryLoop => {
                Self::deliver_to_all_idle(this).await;
            }
        }
    }

    async fn deliver_to_all_idle(this: &Arc<Self>) {
        loop {
            let mut infra = this.infra.lock().await;
            let idle_targets: Vec<AgentId> = infra
                .leaders
                .iter()
                .chain(infra.members.iter())
                .filter(|m| m.status == AgentStatus::Idle)
                .map(|m| m.id.clone())
                .collect();

            let mut any_delivered = false;
            for target_id in &idle_targets {
                match deliver_queued_messages(target_id, &this.db, &mut infra, &*this.tmux) {
                    Ok(true) => any_delivered = true,
                    Ok(false) => {}
                    Err(e) => {
                        tracing::error!(
                            target_id = %target_id,
                            error = %e,
                            "delivery loop: failed to deliver"
                        );
                    }
                }
            }
            if !any_delivered {
                break;
            }
        }
    }

    fn spawn_readiness_watcher(this: &Arc<Self>, target_id: AgentId) {
        let this = Arc::clone(this);
        let max_polls = READINESS_TIMEOUT.as_secs() / READINESS_POLL_INTERVAL.as_secs();

        tokio::spawn(async move {
            for _ in 0..max_polls {
                tokio::time::sleep(READINESS_POLL_INTERVAL).await;

                let terminal_target = {
                    let infra = this.infra.lock().await;
                    let agent = infra
                        .find_member(&target_id)
                        .or_else(|| infra.find_leader(&target_id));
                    match agent {
                        Some(m) => m.terminal_target.clone(),
                        None => return,
                    }
                };

                use palette_tmux::TerminalManager as _;
                let pane_content = match this.tmux.capture_pane(&terminal_target) {
                    Ok(content) => content,
                    Err(e) => {
                        tracing::warn!(
                            target_id = %target_id,
                            error = %e,
                            "failed to capture pane"
                        );
                        continue;
                    }
                };

                if !pane_content.contains('❯') {
                    continue;
                }

                tracing::info!(
                    target_id = %target_id,
                    "Claude Code is ready, delivering queued message"
                );

                {
                    let mut infra = this.infra.lock().await;
                    let is_booting = infra
                        .find_member(&target_id)
                        .or_else(|| infra.find_leader(&target_id))
                        .is_some_and(|m| m.status == AgentStatus::Booting);
                    if is_booting {
                        if let Some(m) = infra.find_member_mut(&target_id) {
                            m.status = AgentStatus::Idle;
                        } else if let Some(m) = infra.find_leader_mut(&target_id) {
                            m.status = AgentStatus::Idle;
                        }
                        infra.touch();
                    }
                    let _ = deliver_queued_messages(&target_id, &this.db, &mut infra, &*this.tmux);
                    Self::save_state(&this, &infra);
                }
                return;
            }

            tracing::error!(
                target_id = %target_id,
                "timed out waiting for Claude Code readiness"
            );
        });
    }

    fn save_state(this: &Arc<Self>, infra: &PersistentState) {
        let path = std::path::PathBuf::from(&this.state_path);
        if let Err(e) = palette_file_state::save(infra, &path) {
            tracing::error!(error = %e, "failed to save state");
        }
    }
}
