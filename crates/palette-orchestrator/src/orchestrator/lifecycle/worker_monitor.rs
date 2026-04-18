use super::Orchestrator;
use palette_domain::worker::{WorkerId, WorkerRole, WorkerState, WorkerStatus};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Base interval between monitoring polls (jittered by up to 500ms).
/// Jitter serves two purposes:
/// - Prevents thundering herd when monitoring many workers simultaneously
/// - Avoids aliasing with periodic UI updates (e.g. loading spinners) that
///   could cause false stall detection if capture intervals synchronize
const MONITOR_POLL_BASE: Duration = Duration::from_millis(2500);

/// Maximum random jitter added to each poll interval.
const MONITOR_POLL_JITTER: Duration = Duration::from_millis(500);

/// Number of poll cycles between liveness checks (~9s / ~3s = 3).
const LIVENESS_CHECK_EVERY: u64 = 3;

/// Time without pane content change before a worker is considered stalled.
/// Claude Code constantly updates the screen while thinking/working, so
/// 10 seconds of no change reliably indicates a permission prompt or hang.
const STALL_TIMEOUT: Duration = Duration::from_secs(10);

/// Interval between repeated stall/auth-error alerts for the same worker.
/// Ensures long-running issues remain visible in logs rather than being
/// reported once and forgotten.
const REALERT_INTERVAL: Duration = Duration::from_secs(60);

/// Maximum crash recovery attempts per worker before escalation.
const CRASH_RETRY_LIMIT: u32 = 3;

/// Tracks pane content hash and last change time for stall/auth-error detection.
struct PaneSnapshot {
    hash: u64,
    last_changed: Instant,
    stall_alerted_at: Option<Instant>,
    auth_error_alerted_at: Option<Instant>,
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
                let jitter =
                    Duration::from_millis(rand_u64() % MONITOR_POLL_JITTER.as_millis() as u64);
                let interval = MONITOR_POLL_BASE + jitter;
                tokio::select! {
                    () = this.cancel_token.cancelled() => {
                        tracing::info!("worker monitor stopped (shutdown)");
                        break;
                    }
                    () = tokio::time::sleep(interval) => {
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
        let workers = match self.interactor.data_store.list_all_workers() {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "monitor: failed to list workers");
                return;
            }
        };

        // Clean up snapshots/retries for workers that no longer exist
        let active_ids: HashSet<_> = workers.iter().map(|w| w.id.clone()).collect();
        pane_snapshots.retain(|id, _| active_ids.contains(id));
        crash_retries.retain(|id, _| active_ids.contains(id));

        for worker in &workers {
            match worker.status {
                // Readiness watcher handles Booting workers
                WorkerStatus::Booting => continue,
                // Already handling crash recovery
                WorkerStatus::Crashed => continue,
                // Suspended workers are managed by suspend/resume API
                WorkerStatus::Suspended => continue,
                WorkerStatus::Working | WorkerStatus::Idle | WorkerStatus::WaitingPermission => {
                    // Skip workers whose workflow is Suspending — their containers may be
                    // stopped but DB status not yet updated to Suspended. Without this check,
                    // the monitor would misinterpret the stopped container as a crash.
                    if self
                        .is_workflow_suspending(&worker.workflow_id)
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    if check_liveness
                        && !self
                            .interactor
                            .container
                            .is_container_running(worker.container_id.as_ref())
                    {
                        self.handle_crash(worker, crash_retries);
                        continue;
                    }
                    self.check_stall(worker, pane_snapshots);
                }
            }
        }

        // All-member-idle detection (per supervisor)
        self.check_all_members_idle(&workers);

        // Worker-limit deadlock detection
        self.check_worker_limit_deadlock(&workers);
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
            .interactor
            .data_store
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
        if let Err(e) = self
            .interactor
            .container
            .start_container(&worker.container_id)
        {
            tracing::error!(
                error = %e,
                worker_id = %worker.id,
                "failed to restart container during crash recovery"
            );
            return;
        }

        // Send resume or fresh start command
        let cmd = if let Some(ref session_id) = worker.session_id {
            self.interactor.container.claude_resume_command(
                &worker.container_id,
                session_id,
                worker.role,
                None,
            )
        } else {
            // No session_id: start fresh with prompt file
            let prompt_file = if worker.role.is_supervisor() {
                // Use supervisor prompt for recovery (the original prompt file path is not stored)
                "/home/agent/prompt.md"
            } else {
                "/home/agent/prompt.md"
            };
            self.interactor.container.claude_exec_command(
                &worker.container_id,
                prompt_file,
                worker.role,
                None,
            )
        };

        if let Err(e) = self
            .interactor
            .terminal
            .send_keys(&worker.terminal_target, &cmd)
        {
            tracing::error!(
                error = %e,
                worker_id = %worker.id,
                "failed to send recovery command"
            );
            return;
        }

        // Update status to Booting and let readiness watcher handle the rest
        if let Err(e) = self
            .interactor
            .data_store
            .update_worker_status(&worker.id, WorkerStatus::Booting)
        {
            tracing::error!(error = %e, worker_id = %worker.id, "failed to update status to Booting");
            return;
        }

        self.spawn_readiness_watcher(worker.id.clone());

        // Alert the supervisor (for members only)
        if worker.role == WorkerRole::Member
            && let Some(ref supervisor_id) = worker.supervisor_id
        {
            let alert = format!(
                "[alert] member={} type=crash_recovery attempt={}",
                worker.id, *retries,
            );
            if let Err(e) = self
                .interactor
                .data_store
                .enqueue_message(supervisor_id, &alert)
            {
                tracing::error!(error = %e, "failed to enqueue crash alert for supervisor");
            }
        }

        tracing::info!(worker_id = %worker.id, "crash recovery initiated");
    }

    /// Escalate when crash recovery retries are exhausted.
    fn escalate_crash(&self, worker: &WorkerState) {
        if worker.role == WorkerRole::Member {
            if let Some(ref supervisor_id) = worker.supervisor_id
                && let Err(e) = self.interactor.data_store.enqueue_message(
                    supervisor_id,
                    &format!(
                        "[alert] member={} type=crash_unrecoverable retries_exhausted=true",
                        worker.id,
                    ),
                )
            {
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

    /// Check if a worker's pane content has changed; detect stalls and auth errors.
    fn check_stall(&self, worker: &WorkerState, snapshots: &mut HashMap<WorkerId, PaneSnapshot>) {
        // Only check stalls for actively working workers
        if worker.status != WorkerStatus::Working {
            return;
        }

        let pane_content = match self
            .interactor
            .terminal
            .capture_pane(&worker.terminal_target)
        {
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
            stall_alerted_at: None,
            auth_error_alerted_at: None,
        });

        if snapshot.hash != hash {
            snapshot.hash = hash;
            snapshot.last_changed = now;
            snapshot.stall_alerted_at = None;
            snapshot.auth_error_alerted_at = None;
            return;
        }

        // Check for authentication error before generic stall detection.
        // When auth error is detected, skip stall alert (auth error is more specific).
        if pane_content.contains("authentication_error") {
            let should_alert = match snapshot.auth_error_alerted_at {
                None => true,
                Some(last_alert) => now.duration_since(last_alert) >= REALERT_INTERVAL,
            };
            if should_alert {
                snapshot.auth_error_alerted_at = Some(now);
                tracing::error!(
                    worker_id = %worker.id,
                    role = %worker.role,
                    "authentication error detected: worker credentials have expired. \
                     Run /palette:login to refresh the auth token",
                );
            }
            return;
        }

        // Hash unchanged — check if stall timeout exceeded
        let stalled_duration = now.duration_since(snapshot.last_changed);
        if stalled_duration < STALL_TIMEOUT {
            return;
        }

        let should_alert = match snapshot.stall_alerted_at {
            None => true,
            Some(last_alert) => now.duration_since(last_alert) >= REALERT_INTERVAL,
        };

        if should_alert {
            snapshot.stall_alerted_at = Some(now);

            tracing::warn!(
                worker_id = %worker.id,
                role = %worker.role,
                stalled_secs = stalled_duration.as_secs(),
                "stall detected: pane unchanged for {stalled_duration:?}",
            );
        }
    }

    /// Detect when assignable jobs exist but cannot be dispatched because
    /// the worker limit is reached and no existing worker is making progress.
    fn check_worker_limit_deadlock(&self, workers: &[WorkerState]) {
        let active = match self.interactor.data_store.count_active_workers() {
            Ok(n) => n,
            Err(e) => {
                tracing::error!(error = %e, "monitor: failed to count active workers");
                return;
            }
        };
        if active < self.docker_config.max_workers {
            return;
        }

        let assignable = match self.interactor.data_store.find_assignable_jobs() {
            Ok(jobs) => jobs,
            Err(e) => {
                tracing::error!(error = %e, "monitor: failed to find assignable jobs");
                return;
            }
        };
        if assignable.is_empty() {
            return;
        }

        // All worker slots are full and jobs are waiting.
        // Check if any worker is actively making progress.
        let any_working = workers
            .iter()
            .any(|w| matches!(w.status, WorkerStatus::Working | WorkerStatus::Booting));

        if !any_working {
            tracing::warn!(
                active_workers = active,
                max_workers = self.docker_config.max_workers,
                assignable_jobs = assignable.len(),
                "worker limit deadlock: assignable jobs exist but all worker slots are \
                 occupied and no worker is actively making progress"
            );
        }
    }

    /// Detect when all members under a supervisor are idle but work remains.
    fn check_all_members_idle(&self, workers: &[WorkerState]) {
        // Group members by supervisor
        let mut supervisor_members: HashMap<WorkerId, Vec<&WorkerState>> = HashMap::new();
        for w in workers {
            if w.role == WorkerRole::Member
                && let Some(ref supervisor_id) = w.supervisor_id
            {
                supervisor_members
                    .entry(supervisor_id.clone())
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
            let has_pending = members.iter().any(|m| {
                self.interactor
                    .data_store
                    .has_pending_messages(&m.id)
                    .unwrap_or(false)
            });

            if has_pending {
                tracing::warn!(
                    supervisor_id = %supervisor_id,
                    member_count = members.len(),
                    "all members idle/stopped but pending messages exist"
                );
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

/// Cheap pseudo-random u64 using thread-local state seeded from system time.
/// Not cryptographic — only used for jittering poll intervals.
fn rand_u64() -> u64 {
    use std::cell::Cell;
    thread_local! {
        static STATE: Cell<u64> = Cell::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64
        );
    }
    STATE.with(|s| {
        // xorshift64
        let mut x = s.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        x
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;
    use palette_domain::job::Job;
    use palette_usecase::Interactor;
    use std::collections::HashMap;
    use tokio_util::sync::CancellationToken;

    fn make_orchestrator(
        data_store: MockDataStore,
        container: MockContainerRuntime,
        terminal: MockTerminalSession,
    ) -> Arc<Orchestrator> {
        Arc::new(Orchestrator {
            interactor: Arc::new(Interactor {
                container: Box::new(container),
                terminal: Box::new(terminal),
                data_store: Box::new(data_store),
                blueprint: Box::new(MockBlueprintReader),
                github_review_port: Box::new(MockGitHubReview),
            }),
            docker_config: crate::DockerConfig {
                worker_callback_url: String::new(),
                callback_network: crate::CallbackNetwork::Auto,
                approver_image: String::new(),
                member_image: String::new(),
                settings_template: String::new(),
                approver_prompt: String::new(),
                review_integrator_image: String::new(),
                review_integrator_prompt: String::new(),
                crafter_prompt: String::new(),
                reviewer_prompt: String::new(),
                max_workers: 3,
            },
            session_name: String::new(),
            cancel_token: CancellationToken::new(),
            workspace_manager: crate::orchestrator::infra::workspace::WorkspaceManager::new("data"),
            perspectives: crate::ValidatedPerspectives {
                dirs: HashMap::new(),
                perspectives: vec![],
            },
            event_tx: tokio::sync::mpsc::unbounded_channel().0,
        })
    }

    #[test]
    fn hash_detects_change() {
        let h1 = hash_string("hello world");
        let h2 = hash_string("hello world");
        let h3 = hash_string("hello world!");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    // -- Crash detection tests --

    #[tokio::test]
    async fn crash_detected_updates_status_and_starts_recovery() {
        let worker = make_worker("m-1", WorkerRole::Member, WorkerStatus::Working);
        let data_store = MockDataStore::with_workers(vec![worker]);
        // Container is NOT running → crash detected
        let container = MockContainerRuntime::new();
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);
        let mut snapshots = HashMap::new();
        let mut retries = HashMap::new();

        orch.check_all_workers(true, &mut snapshots, &mut retries);

        // Crash should be detected and recovery attempted
        assert_eq!(*retries.get(&WorkerId::parse("m-1").unwrap()).unwrap(), 1);

        // Worker status should have been updated (Crashed then Booting)
        let worker = orch
            .interactor
            .data_store
            .find_worker(&WorkerId::parse("m-1").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(worker.status, WorkerStatus::Booting);
    }

    #[tokio::test]
    async fn crash_recovery_retries_exhausted_escalates() {
        let worker = make_worker("m-1", WorkerRole::Member, WorkerStatus::Working);
        let data_store = MockDataStore::with_workers(vec![worker]);
        let container = MockContainerRuntime::new();
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);
        let mut snapshots = HashMap::new();
        let mut retries = HashMap::new();

        // Exhaust retries (CRASH_RETRY_LIMIT = 3)
        for _ in 0..4 {
            // Reset worker status to Working for next iteration
            let _ = orch
                .interactor
                .data_store
                .update_worker_status(&WorkerId::parse("m-1").unwrap(), WorkerStatus::Working);
            orch.check_all_workers(true, &mut snapshots, &mut retries);
        }

        // After 4 attempts, retries should be 4 (> CRASH_RETRY_LIMIT of 3)
        assert_eq!(*retries.get(&WorkerId::parse("m-1").unwrap()).unwrap(), 4);

        // Escalation message should have been enqueued to supervisor
        let messages = orch
            .interactor
            .data_store
            .has_pending_messages(&WorkerId::parse("sup-1").unwrap())
            .unwrap();
        assert!(
            messages,
            "escalation message should be enqueued to supervisor"
        );
    }

    // -- Stall detection tests --

    #[test]
    fn stall_not_detected_when_pane_changes() {
        let worker = make_worker("m-1", WorkerRole::Member, WorkerStatus::Working);
        let data_store = MockDataStore::with_workers(vec![worker]);
        let container = MockContainerRuntime::with_running(&["container-m-1"]);
        let terminal = MockTerminalSession::new();
        terminal.set_pane_content("pane-m-1", "output line 1");

        let orch = make_orchestrator(data_store, container, terminal);
        let mut snapshots = HashMap::new();
        let mut retries = HashMap::new();

        // First check: establishes baseline
        orch.check_all_workers(false, &mut snapshots, &mut retries);
        assert!(snapshots.contains_key(&WorkerId::parse("m-1").unwrap()));
        assert!(
            snapshots[&WorkerId::parse("m-1").unwrap()]
                .stall_alerted_at
                .is_none()
        );
    }

    #[test]
    fn stall_detected_when_pane_unchanged_past_timeout() {
        let worker = make_worker("m-1", WorkerRole::Member, WorkerStatus::Working);
        let data_store = MockDataStore::with_workers(vec![worker]);
        let container = MockContainerRuntime::with_running(&["container-m-1"]);
        let terminal = MockTerminalSession::new();
        terminal.set_pane_content("pane-m-1", "stuck output");

        let orch = make_orchestrator(data_store, container, terminal);
        let mut snapshots = HashMap::new();
        let mut retries = HashMap::new();

        // First check: establishes baseline
        orch.check_all_workers(false, &mut snapshots, &mut retries);

        // Manually backdate the snapshot to simulate time passing
        snapshots
            .get_mut(&WorkerId::parse("m-1").unwrap())
            .unwrap()
            .last_changed = Instant::now() - STALL_TIMEOUT - Duration::from_secs(1);

        // Second check: same content + timeout exceeded → stall detected
        orch.check_all_workers(false, &mut snapshots, &mut retries);
        assert!(
            snapshots[&WorkerId::parse("m-1").unwrap()]
                .stall_alerted_at
                .is_some()
        );
    }

    #[test]
    fn stall_realert_after_interval() {
        let worker = make_worker("m-1", WorkerRole::Member, WorkerStatus::Working);
        let data_store = MockDataStore::with_workers(vec![worker]);
        let container = MockContainerRuntime::with_running(&["container-m-1"]);
        let terminal = MockTerminalSession::new();
        terminal.set_pane_content("pane-m-1", "stuck output");

        let orch = make_orchestrator(data_store, container, terminal);
        let mut snapshots = HashMap::new();
        let mut retries = HashMap::new();

        // First check: establishes baseline
        orch.check_all_workers(false, &mut snapshots, &mut retries);

        // Backdate to trigger initial stall alert
        let stall_start = Instant::now() - REALERT_INTERVAL - STALL_TIMEOUT;
        snapshots
            .get_mut(&WorkerId::parse("m-1").unwrap())
            .unwrap()
            .last_changed = stall_start;

        orch.check_all_workers(false, &mut snapshots, &mut retries);
        let first_alert = snapshots[&WorkerId::parse("m-1").unwrap()]
            .stall_alerted_at
            .unwrap();

        // Immediately check again — should NOT re-alert (interval not elapsed)
        orch.check_all_workers(false, &mut snapshots, &mut retries);
        let same_alert = snapshots[&WorkerId::parse("m-1").unwrap()]
            .stall_alerted_at
            .unwrap();
        assert_eq!(
            first_alert, same_alert,
            "should not re-alert before interval elapses"
        );

        // Backdate the alert time to simulate interval passing
        snapshots
            .get_mut(&WorkerId::parse("m-1").unwrap())
            .unwrap()
            .stall_alerted_at = Some(Instant::now() - REALERT_INTERVAL);

        orch.check_all_workers(false, &mut snapshots, &mut retries);
        let second_alert = snapshots[&WorkerId::parse("m-1").unwrap()]
            .stall_alerted_at
            .unwrap();
        assert_ne!(
            first_alert, second_alert,
            "should re-alert after interval elapses"
        );
    }

    #[test]
    fn stall_realert_resets_on_pane_change() {
        let worker = make_worker("m-1", WorkerRole::Member, WorkerStatus::Working);
        let data_store = MockDataStore::with_workers(vec![worker]);
        let container = MockContainerRuntime::with_running(&["container-m-1"]);
        let terminal = MockTerminalSession::new();
        terminal.set_pane_content("pane-m-1", "stuck output");

        let orch = make_orchestrator(data_store, container, terminal);
        let mut snapshots = HashMap::new();
        let mut retries = HashMap::new();

        // Trigger a stall alert
        orch.check_all_workers(false, &mut snapshots, &mut retries);
        snapshots
            .get_mut(&WorkerId::parse("m-1").unwrap())
            .unwrap()
            .last_changed = Instant::now() - STALL_TIMEOUT - Duration::from_secs(1);
        orch.check_all_workers(false, &mut snapshots, &mut retries);
        assert!(
            snapshots[&WorkerId::parse("m-1").unwrap()]
                .stall_alerted_at
                .is_some()
        );

        // Simulate pane content change by altering the snapshot hash directly.
        // When check_stall captures a different hash, it resets the alert.
        snapshots
            .get_mut(&WorkerId::parse("m-1").unwrap())
            .unwrap()
            .hash = 0;
        orch.check_all_workers(false, &mut snapshots, &mut retries);
        assert!(
            snapshots[&WorkerId::parse("m-1").unwrap()]
                .stall_alerted_at
                .is_none(),
            "alert should reset when pane content changes"
        );
    }

    #[test]
    fn stall_only_checked_for_working_status() {
        // Idle worker should NOT be checked for stall
        let worker = make_worker("m-1", WorkerRole::Member, WorkerStatus::Idle);
        let data_store = MockDataStore::with_workers(vec![worker]);
        let container = MockContainerRuntime::with_running(&["container-m-1"]);
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);
        let mut snapshots = HashMap::new();
        let mut retries = HashMap::new();

        orch.check_all_workers(false, &mut snapshots, &mut retries);

        // No snapshot created for idle worker
        assert!(!snapshots.contains_key(&WorkerId::parse("m-1").unwrap()));
    }

    // -- Authentication error detection tests --

    #[test]
    fn auth_error_detected_from_pane_content() {
        let worker = make_worker("m-1", WorkerRole::Member, WorkerStatus::Working);
        let data_store = MockDataStore::with_workers(vec![worker]);
        let container = MockContainerRuntime::with_running(&["container-m-1"]);
        let terminal = MockTerminalSession::new();
        terminal.set_pane_content(
            "pane-m-1",
            r#"{"type":"error","error":{"type":"authentication_error","message":"Invalid authentication credentials"}}"#,
        );

        let orch = make_orchestrator(data_store, container, terminal);
        let mut snapshots = HashMap::new();
        let mut retries = HashMap::new();

        // First check: establishes baseline
        orch.check_all_workers(false, &mut snapshots, &mut retries);

        // Second check: same content, auth error should be detected
        orch.check_all_workers(false, &mut snapshots, &mut retries);
        let snapshot = &snapshots[&WorkerId::parse("m-1").unwrap()];
        assert!(
            snapshot.auth_error_alerted_at.is_some(),
            "auth error should be detected"
        );
        // Stall should NOT be alerted (auth error takes precedence)
        assert!(
            snapshot.stall_alerted_at.is_none(),
            "stall alert should not fire when auth error is detected"
        );
    }

    #[test]
    fn auth_error_resets_on_pane_change() {
        let worker = make_worker("m-1", WorkerRole::Member, WorkerStatus::Working);
        let data_store = MockDataStore::with_workers(vec![worker]);
        let container = MockContainerRuntime::with_running(&["container-m-1"]);
        let terminal = MockTerminalSession::new();
        terminal.set_pane_content(
            "pane-m-1",
            r#"{"type":"error","error":{"type":"authentication_error","message":"Invalid authentication credentials"}}"#,
        );

        let orch = make_orchestrator(data_store, container, terminal);
        let mut snapshots = HashMap::new();
        let mut retries = HashMap::new();

        // Trigger auth error detection
        orch.check_all_workers(false, &mut snapshots, &mut retries);
        orch.check_all_workers(false, &mut snapshots, &mut retries);
        assert!(
            snapshots[&WorkerId::parse("m-1").unwrap()]
                .auth_error_alerted_at
                .is_some()
        );

        // Simulate pane content change (worker recovered)
        snapshots
            .get_mut(&WorkerId::parse("m-1").unwrap())
            .unwrap()
            .hash = 0;
        orch.check_all_workers(false, &mut snapshots, &mut retries);
        assert!(
            snapshots[&WorkerId::parse("m-1").unwrap()]
                .auth_error_alerted_at
                .is_none(),
            "auth error alert should reset when pane content changes"
        );
    }

    // -- All-idle detection tests --

    #[test]
    fn all_idle_detected_when_pending_messages_exist() {
        let member = make_worker("m-1", WorkerRole::Member, WorkerStatus::Idle);
        let data_store = MockDataStore::with_workers(vec![member]);
        // Pre-enqueue a message so has_pending_messages returns true
        data_store.messages.lock().unwrap().insert(
            WorkerId::parse("m-1").unwrap(),
            vec!["pending work".to_string()],
        );

        let container = MockContainerRuntime::with_running(&["container-m-1"]);
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);
        let mut snapshots = HashMap::new();
        let mut retries = HashMap::new();

        // This should log a warning about all-idle with pending messages.
        // We verify it doesn't panic and the logic runs correctly.
        orch.check_all_workers(false, &mut snapshots, &mut retries);
    }

    #[test]
    fn all_idle_not_triggered_when_member_is_working() {
        let idle = make_worker("m-1", WorkerRole::Member, WorkerStatus::Idle);
        let working = make_worker("m-2", WorkerRole::Member, WorkerStatus::Working);
        let data_store = MockDataStore::with_workers(vec![idle, working]);
        data_store.messages.lock().unwrap().insert(
            WorkerId::parse("m-1").unwrap(),
            vec!["pending work".to_string()],
        );

        let container = MockContainerRuntime::with_running(&["container-m-1", "container-m-2"]);
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);
        let mut snapshots = HashMap::new();
        let mut retries = HashMap::new();

        // With one working member, all-idle should NOT trigger
        orch.check_all_workers(false, &mut snapshots, &mut retries);
    }

    // -- Worker-limit deadlock detection tests --

    fn make_todo_job(id: &str) -> Job {
        crate::testing::make_job(id)
    }

    #[test]
    fn deadlock_detected_when_all_slots_full_and_jobs_waiting() {
        // 3 workers (= max_workers), all idle, with an assignable job
        let approver1 = make_worker("approver-1", WorkerRole::Approver, WorkerStatus::Idle);
        let approver2 = make_worker("approver-2", WorkerRole::Approver, WorkerStatus::Idle);
        let member1 = make_worker("m-1", WorkerRole::Member, WorkerStatus::Idle);
        let data_store = MockDataStore::with_workers(vec![approver1, approver2, member1]);
        data_store
            .assignable_jobs
            .lock()
            .unwrap()
            .push(make_todo_job("R-1"));

        let container = MockContainerRuntime::with_running(&[
            "container-approver-1",
            "container-approver-2",
            "container-m-1",
        ]);
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);
        let mut snapshots = HashMap::new();
        let mut retries = HashMap::new();

        // This should log a deadlock warning (we verify it doesn't panic)
        orch.check_all_workers(false, &mut snapshots, &mut retries);
    }

    #[test]
    fn deadlock_not_triggered_when_worker_is_making_progress() {
        // 3 workers (= max_workers), one is Working → no deadlock
        let approver1 = make_worker("approver-1", WorkerRole::Approver, WorkerStatus::Idle);
        let approver2 = make_worker("approver-2", WorkerRole::Approver, WorkerStatus::Idle);
        let member1 = make_worker("m-1", WorkerRole::Member, WorkerStatus::Working);
        let data_store = MockDataStore::with_workers(vec![approver1, approver2, member1]);
        data_store
            .assignable_jobs
            .lock()
            .unwrap()
            .push(make_todo_job("R-1"));

        let container = MockContainerRuntime::with_running(&[
            "container-approver-1",
            "container-approver-2",
            "container-m-1",
        ]);
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);
        let mut snapshots = HashMap::new();
        let mut retries = HashMap::new();

        // Working member means progress is being made → no deadlock
        orch.check_all_workers(false, &mut snapshots, &mut retries);
    }

    #[test]
    fn deadlock_not_triggered_when_slots_available() {
        // 2 workers but max_workers=3 → slot available
        let approver1 = make_worker("approver-1", WorkerRole::Approver, WorkerStatus::Idle);
        let member1 = make_worker("m-1", WorkerRole::Member, WorkerStatus::Idle);
        let data_store = MockDataStore::with_workers(vec![approver1, member1]);
        data_store
            .assignable_jobs
            .lock()
            .unwrap()
            .push(make_todo_job("R-1"));

        let container =
            MockContainerRuntime::with_running(&["container-approver-1", "container-m-1"]);
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);
        let mut snapshots = HashMap::new();
        let mut retries = HashMap::new();

        // Slots available → no deadlock
        orch.check_all_workers(false, &mut snapshots, &mut retries);
    }

    #[test]
    fn booting_crashed_and_suspended_workers_skipped_in_liveness_check() {
        let booting = make_worker("m-1", WorkerRole::Member, WorkerStatus::Booting);
        let crashed = make_worker("m-2", WorkerRole::Member, WorkerStatus::Crashed);
        let suspended = make_worker("m-3", WorkerRole::Member, WorkerStatus::Suspended);
        let data_store = MockDataStore::with_workers(vec![booting, crashed, suspended]);
        // No container is running, but they should all be skipped
        let container = MockContainerRuntime::new();
        let terminal = MockTerminalSession::new();

        let orch = make_orchestrator(data_store, container, terminal);
        let mut snapshots = HashMap::new();
        let mut retries = HashMap::new();

        orch.check_all_workers(true, &mut snapshots, &mut retries);

        // No crash retries should be recorded (all were skipped)
        assert!(retries.is_empty());
    }
}
