use super::Orchestrator;
use palette_domain::agent::{AgentId, AgentStatus};
use std::sync::Arc;

use crate::deliver_queued_messages;

/// Interval between readiness polls.
const READINESS_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3);

/// Maximum time to wait for Claude Code readiness.
const READINESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

impl Orchestrator {
    pub(super) fn spawn_readiness_watcher(self: &Arc<Self>, target_id: AgentId) {
        let this = Arc::clone(self);
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
                    let _ = deliver_queued_messages(&target_id, &this.db, &mut infra, &this.tmux);
                    this.save_state(&infra);
                }
                return;
            }

            tracing::error!(
                target_id = %target_id,
                "timed out waiting for Claude Code readiness"
            );
        });
    }
}
