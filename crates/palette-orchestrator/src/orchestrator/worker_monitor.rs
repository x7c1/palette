use super::Orchestrator;
use palette_docker::{DockerManager, is_container_running};
use palette_domain::worker::{WorkerId, WorkerRole, WorkerState, WorkerStatus};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Interval between monitoring polls.
const MONITOR_POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Number of poll cycles between liveness checks (15s / 5s = 3).
const LIVENESS_CHECK_EVERY: u64 = 3;

/// Time without pane content change before a worker is considered stalled.
const STALL_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum crash recovery attempts per worker before escalation.
const CRASH_RETRY_LIMIT: u32 = 3;

/// Tracks pane content hash and last change time for stall detection.
struct PaneSnapshot {
    hash: u64,
    last_changed: Instant,
    stall_alerted: bool,
}

impl Orchestrator {
    /// Start the worker monitoring loop.
    ///
    /// Periodically checks all workers for crashes, stalls, and all-idle conditions.
    /// Stops when the cancellation token is cancelled (during shutdown).
    pub fn spawn_worker_monitor(self: &Arc<Self>) {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let mut poll_count: u64 = 0;
            let mut pane_snapshots: HashMap<WorkerId, PaneSnapshot> = HashMap::new();
            let mut crash_retries: HashMap<WorkerId, u32> = HashMap::new();

            loop {
                tokio::select! {
                    () = this.cancel_token.cancelled() => {
                        tracing::info!("worker monitor stopped (shutdown)");
                        break;
                    }
                    () = tokio::time::sleep(MONITOR_POLL_INTERVAL) => {
                        poll_count += 1;
                        let check_liveness = poll_count.is_multiple_of(LIVENESS_CHECK_EVERY);
                        this.check_all_workers(
                            check_liveness,
                            &mut pane_snapshots,
                            &mut crash_retries,
                        );
                    }
                }
            }
        });
    }

    /// Run a single monitoring check on all workers.
    ///
    /// Exposed as a separate method so it can be called on-demand (e.g., by 003
    /// orchestrator recovery at startup).
    pub fn run_health_check(self: &Arc<Self>) {
        let mut pane_snapshots = HashMap::new();
        let mut crash_retries = HashMap::new();
        self.check_all_workers(true, &mut pane_snapshots, &mut crash_retries);
    }

    fn check_all_workers(
        self: &Arc<Self>,
        check_liveness: bool,
        pane_snapshots: &mut HashMap<WorkerId, PaneSnapshot>,
        crash_retries: &mut HashMap<WorkerId, u32>,
    ) {
        let workers = match self.db.list_all_workers() {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "monitor: failed to list workers");
                return;
            }
        };

        // Clean up snapshots/retries for workers that no longer exist
        let active_ids: std::collections::HashSet<_> =
            workers.iter().map(|w| w.id.clone()).collect();
        pane_snapshots.retain(|id, _| active_ids.contains(id));
        crash_retries.retain(|id, _| active_ids.contains(id));

        for worker in &workers {
            match worker.status {
                // Readiness watcher handles Booting workers
                WorkerStatus::Booting => continue,
                // Already handling crash recovery
                WorkerStatus::Crashed => continue,
                WorkerStatus::Working | WorkerStatus::Idle | WorkerStatus::WaitingPermission => {
                    if check_liveness && !is_container_running(worker.container_id.as_ref()) {
                        self.handle_crash(worker, crash_retries);
                        continue;
                    }
                    self.check_stall(worker, pane_snapshots);
                }
            }
        }

        // All-member-idle detection (per supervisor)
        self.check_all_members_idle(&workers);
    }

    /// Handle a crashed worker: update status, attempt recovery.
    fn handle_crash(
        self: &Arc<Self>,
        worker: &WorkerState,
        crash_retries: &mut HashMap<WorkerId, u32>,
    ) {
        tracing::warn!(
            worker_id = %worker.id,
            role = %worker.role,
            "crash detected: container is not running"
        );

        if let Err(e) = self
            .db
            .update_worker_status(&worker.id, WorkerStatus::Crashed)
        {
            tracing::error!(error = %e, worker_id = %worker.id, "failed to update status to Crashed");
            return;
        }

        let retries = crash_retries.entry(worker.id.clone()).or_insert(0);
        *retries += 1;

        if *retries > CRASH_RETRY_LIMIT {
            self.escalate_crash(worker);
            return;
        }

        tracing::info!(
            worker_id = %worker.id,
            attempt = *retries,
            "attempting crash recovery"
        );

        // Restart the stopped container
        if let Err(e) = self.docker.start_container(&worker.container_id) {
            tracing::error!(
                error = %e,
                worker_id = %worker.id,
                "failed to restart container during crash recovery"
            );
            return;
        }

        // Send resume or fresh start command
        let cmd = if let Some(ref session_id) = worker.session_id {
            DockerManager::claude_resume_command(&worker.container_id, session_id, worker.role)
        } else {
            // No session_id: start fresh with prompt file
            let prompt_file = if worker.role.is_supervisor() {
                // Use leader prompt for recovery (the original prompt file path is not stored)
                "/home/agent/prompt.md"
            } else {
                "/home/agent/prompt.md"
            };
            DockerManager::claude_exec_command(&worker.container_id, prompt_file, worker.role)
        };

        if let Err(e) = self.tmux.send_keys(&worker.terminal_target, &cmd) {
            tracing::error!(
                error = %e,
                worker_id = %worker.id,
                "failed to send recovery command"
            );
            return;
        }

        // Update status to Booting and let readiness watcher handle the rest
        if let Err(e) = self
            .db
            .update_worker_status(&worker.id, WorkerStatus::Booting)
        {
            tracing::error!(error = %e, worker_id = %worker.id, "failed to update status to Booting");
            return;
        }

        self.spawn_readiness_watcher(worker.id.clone());

        // Alert the supervisor (for members only)
        if worker.role == WorkerRole::Member {
            let alert = format!(
                "[alert] member={} type=crash_recovery attempt={}",
                worker.id, *retries,
            );
            if let Err(e) = self.db.enqueue_message(&worker.supervisor_id, &alert) {
                tracing::error!(error = %e, "failed to enqueue crash alert for supervisor");
            }
        }

        tracing::info!(worker_id = %worker.id, "crash recovery initiated");
    }

    /// Escalate when crash recovery retries are exhausted.
    fn escalate_crash(&self, worker: &WorkerState) {
        if worker.role == WorkerRole::Member {
            let alert = format!(
                "[alert] member={} type=crash_unrecoverable retries_exhausted=true",
                worker.id,
            );
            if let Err(e) = self.db.enqueue_message(&worker.supervisor_id, &alert) {
                tracing::error!(error = %e, "failed to enqueue crash escalation");
            }
            tracing::error!(
                worker_id = %worker.id,
                "crash recovery exhausted, escalated to supervisor"
            );
        } else {
            tracing::error!(
                worker_id = %worker.id,
                role = %worker.role,
                "supervisor crash recovery exhausted, operator intervention required"
            );
        }
    }

    /// Check if a worker's pane content has changed; detect stalls.
    fn check_stall(&self, worker: &WorkerState, snapshots: &mut HashMap<WorkerId, PaneSnapshot>) {
        // Only check stalls for actively working workers
        if worker.status != WorkerStatus::Working {
            return;
        }

        let pane_content = match self.tmux.capture_pane(&worker.terminal_target) {
            Ok(content) => content,
            Err(e) => {
                tracing::warn!(
                    worker_id = %worker.id,
                    error = %e,
                    "monitor: failed to capture pane for stall check"
                );
                return;
            }
        };

        let hash = hash_string(&pane_content);
        let now = Instant::now();

        let snapshot = snapshots.entry(worker.id.clone()).or_insert(PaneSnapshot {
            hash,
            last_changed: now,
            stall_alerted: false,
        });

        if snapshot.hash != hash {
            snapshot.hash = hash;
            snapshot.last_changed = now;
            snapshot.stall_alerted = false;
            return;
        }

        // Hash unchanged — check if stall timeout exceeded
        if !snapshot.stall_alerted && now.duration_since(snapshot.last_changed) >= STALL_TIMEOUT {
            snapshot.stall_alerted = true;

            if worker.role == WorkerRole::Member {
                tracing::warn!(
                    worker_id = %worker.id,
                    "stall detected: member pane unchanged for {:?}",
                    STALL_TIMEOUT,
                );
                let alert = format!(
                    "[alert] member={} type=stall duration={}s",
                    worker.id,
                    STALL_TIMEOUT.as_secs(),
                );
                if let Err(e) = self.db.enqueue_message(&worker.supervisor_id, &alert) {
                    tracing::error!(error = %e, "failed to enqueue stall alert");
                }
            } else {
                tracing::warn!(
                    worker_id = %worker.id,
                    role = %worker.role,
                    "stall detected: supervisor pane unchanged for {:?}, operator intervention may be needed",
                    STALL_TIMEOUT,
                );
            }
        }
    }

    /// Detect when all members under a supervisor are idle but work remains.
    fn check_all_members_idle(&self, workers: &[WorkerState]) {
        // Group members by supervisor
        let mut supervisor_members: HashMap<WorkerId, Vec<&WorkerState>> = HashMap::new();
        for w in workers {
            if w.role == WorkerRole::Member {
                supervisor_members
                    .entry(w.supervisor_id.clone())
                    .or_default()
                    .push(w);
            }
        }

        for (supervisor_id, members) in &supervisor_members {
            // Skip if any member is actively working or booting
            let all_inactive = members.iter().all(|m| {
                matches!(
                    m.status,
                    WorkerStatus::Idle | WorkerStatus::Crashed | WorkerStatus::WaitingPermission
                )
            });

            if !all_inactive {
                continue;
            }

            // Check if any member has pending messages (indicates undelivered work)
            let has_pending = members
                .iter()
                .any(|m| self.db.has_pending_messages(&m.id).unwrap_or(false));

            if has_pending {
                tracing::warn!(
                    supervisor_id = %supervisor_id,
                    member_count = members.len(),
                    "all members idle/stopped but pending messages exist"
                );
                let alert = format!(
                    "[alert] supervisor={supervisor_id} type=all_members_idle member_count={}",
                    members.len(),
                );
                if let Err(e) = self.db.enqueue_message(supervisor_id, &alert) {
                    tracing::error!(error = %e, "failed to enqueue all-idle alert");
                }
            }
        }
    }
}

/// Hash a string using DefaultHasher (not cryptographic, just for change detection).
fn hash_string(s: &str) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_detects_change() {
        let h1 = hash_string("hello world");
        let h2 = hash_string("hello world");
        let h3 = hash_string("hello world!");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn stall_timeout_not_reached() {
        let now = Instant::now();
        let snapshot = PaneSnapshot {
            hash: 42,
            last_changed: now,
            stall_alerted: false,
        };
        // Just created — not stalled
        assert!(now.duration_since(snapshot.last_changed) < STALL_TIMEOUT);
    }

    #[test]
    fn all_inactive_detection() {
        // Test the logic: all members in non-Working states should trigger check
        let statuses = [
            WorkerStatus::Idle,
            WorkerStatus::Crashed,
            WorkerStatus::WaitingPermission,
        ];
        for status in &statuses {
            assert!(matches!(
                status,
                WorkerStatus::Idle | WorkerStatus::Crashed | WorkerStatus::WaitingPermission
            ));
        }
        // Working and Booting should not be considered inactive
        assert!(!matches!(
            WorkerStatus::Working,
            WorkerStatus::Idle | WorkerStatus::Crashed | WorkerStatus::WaitingPermission
        ));
        assert!(!matches!(
            WorkerStatus::Booting,
            WorkerStatus::Idle | WorkerStatus::Crashed | WorkerStatus::WaitingPermission
        ));
    }
}
