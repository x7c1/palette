use crate::AppState;
use palette_domain::server::ServerEvent;
use palette_domain::worker::WorkerStatus;
use std::sync::Arc;
use std::time::Duration;

const CHECK_INTERVAL: Duration = Duration::from_secs(15);

/// Periodically check for pending permission prompts whose supervisor is idle.
///
/// An idle supervisor means it has no queued messages and is not processing
/// anything — so a pending permission prompt was likely lost or ignored.
/// Re-enqueue the notification to give the supervisor another chance.
pub fn spawn_permission_timeout_checker(state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(CHECK_INTERVAL).await;
            check_idle_supervisor_with_pending(&state).await;
        }
    });
}

async fn check_idle_supervisor_with_pending(state: &AppState) {
    let pending: Vec<(String, crate::PendingPermission)> = {
        let events = state.pending_permission_events.lock().await;
        let all: Vec<_> = events.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        if !all.is_empty() {
            tracing::debug!(
                count = all.len(),
                "permission timeout check: pending events"
            );
        }
        all.into_iter()
            .filter(|(_, p)| p.supervisor_id.is_some())
            .collect()
    };

    for (worker_id, perm) in pending {
        let Some(ref supervisor_id) = perm.supervisor_id else {
            continue;
        };

        // Check if the supervisor is idle
        let supervisor = match state.interactor.data_store.find_worker(supervisor_id) {
            Ok(Some(w)) => w,
            _ => continue,
        };

        tracing::debug!(
            worker_id = %worker_id,
            supervisor_id = %supervisor_id,
            supervisor_status = ?supervisor.status,
            "permission timeout check: evaluating"
        );

        if supervisor.status != WorkerStatus::Idle {
            continue;
        }

        // Also check the supervisor has no pending messages
        let has_messages = state
            .interactor
            .data_store
            .has_pending_messages(supervisor_id)
            .unwrap_or(false);
        if has_messages {
            continue;
        }

        // Supervisor is idle with no messages, but a permission prompt is pending.
        tracing::warn!(
            worker_id = %worker_id,
            event_id = %perm.event_id,
            supervisor_id = %supervisor_id,
            "supervisor idle with pending permission prompt, re-sending"
        );

        match state
            .interactor
            .data_store
            .enqueue_message(supervisor_id, &perm.notification)
        {
            Ok(_) => {
                tracing::info!(
                    worker_id = %worker_id,
                    supervisor_id = %supervisor_id,
                    "re-enqueued permission prompt to idle supervisor"
                );
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to re-enqueue permission prompt");
                continue;
            }
        }

        let _ = state.event_tx.send(ServerEvent::NotifyDeliveryLoop);
    }
}
