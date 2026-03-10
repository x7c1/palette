use super::Orchestrator;
use palette_domain::agent::{AgentId, AgentStatus};
use std::ops::ControlFlow;
use std::sync::Arc;

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
                if this.poll_readiness(&target_id).await.is_break() {
                    return;
                }
            }
            tracing::error!(
                target_id = %target_id,
                "timed out waiting for Claude Code readiness"
            );
        });
    }

    /// Poll once for agent readiness.
    /// Returns `Break` if the agent is ready (activated) or gone, `Continue` to keep polling.
    async fn poll_readiness(self: &Arc<Self>, target_id: &AgentId) -> ControlFlow<()> {
        let terminal_target = {
            let infra = self.infra.lock().await;
            let agent = infra
                .find_member(target_id)
                .or_else(|| infra.find_leader(target_id));
            match agent {
                Some(m) => m.terminal_target.clone(),
                None => return ControlFlow::Break(()),
            }
        };

        let pane_content = match self.tmux.capture_pane(&terminal_target) {
            Ok(content) => content,
            Err(e) => {
                tracing::warn!(
                    target_id = %target_id,
                    error = %e,
                    "failed to capture pane"
                );
                return ControlFlow::Continue(());
            }
        };

        if !pane_content.contains('❯') {
            return ControlFlow::Continue(());
        }

        tracing::info!(
            target_id = %target_id,
            "Claude Code is ready, delivering queued message"
        );
        self.activate_agent(target_id).await;
        ControlFlow::Break(())
    }

    /// Transition a booting agent to Idle and deliver queued messages.
    async fn activate_agent(self: &Arc<Self>, target_id: &AgentId) {
        let mut infra = self.infra.lock().await;
        let is_booting = infra
            .find_member(target_id)
            .or_else(|| infra.find_leader(target_id))
            .is_some_and(|m| m.status == AgentStatus::Booting);
        if is_booting {
            if let Some(m) = infra.find_member_mut(target_id) {
                m.status = AgentStatus::Idle;
            } else if let Some(m) = infra.find_leader_mut(target_id) {
                m.status = AgentStatus::Idle;
            }
            infra.touch();
        }
        let _ = self.deliver_queued_messages(target_id, &mut infra);
        self.save_state(&infra);
    }
}
