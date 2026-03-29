use super::Orchestrator;
use palette_domain::worker::{WorkerId, WorkerState, WorkerStatus};
use palette_domain::workflow::WorkflowStatus;
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// Poll interval while waiting for in-progress workers to finish.
const SUSPEND_POLL_INTERVAL: Duration = Duration::from_secs(3);

/// Maximum time to wait for in-progress workers before force-stopping.
const SUSPEND_TIMEOUT: Duration = Duration::from_secs(300);

impl Orchestrator {
    /// Gracefully suspend all active workers.
    ///
    /// 1. Set workflow status to `Suspending` (blocks new job assignment and
    ///    message delivery).
    /// 2. Wait for Working / WaitingPermission members to finish.
    ///    Supervisors stay alive until all their members are done (members
    ///    need supervisor approval for permission prompts).
    /// 3. Suspend idle members as they finish, then supervisors.
    /// 4. On timeout, force-stop remaining workers with a warning.
    /// 5. Set workflow status to `Suspended`.
    pub fn suspend(&self) {
        let workers = match self.interactor.data_store.list_all_workers() {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "suspend: failed to list workers");
                return;
            }
        };

        let suspendable: Vec<_> = workers
            .iter()
            .filter(|w| {
                matches!(
                    w.status,
                    WorkerStatus::Booting
                        | WorkerStatus::Working
                        | WorkerStatus::Idle
                        | WorkerStatus::WaitingPermission
                )
            })
            .collect();

        if suspendable.is_empty() {
            tracing::info!("suspend: no workers to suspend");
            return;
        }

        let workflow_ids: HashSet<_> = suspendable.iter().map(|w| w.workflow_id.clone()).collect();

        // Phase 1: Mark workflows as Suspending to block new assignments/delivery
        for workflow_id in &workflow_ids {
            if let Err(e) = self
                .interactor
                .data_store
                .update_workflow_status(workflow_id, WorkflowStatus::Suspending)
            {
                tracing::warn!(workflow_id = %workflow_id, error = %e, "failed to set Suspending");
            }
        }

        // Partition workers into members and supervisors
        let (members, supervisors): (Vec<&WorkerState>, Vec<&WorkerState>) =
            suspendable.iter().partition(|w| !w.role.is_supervisor());

        // Phase 2: Suspend idle/booting members that have no active work.
        // Working/WaitingPermission members are tracked for the wait loop.
        let mut pending_member_ids: Vec<WorkerId> = Vec::new();
        let mut suspended_count = 0;

        for member in &members {
            match member.status {
                WorkerStatus::Idle | WorkerStatus::Booting => {
                    if self.suspend_single_worker(&member.id, &member.container_id) {
                        suspended_count += 1;
                    }
                }
                WorkerStatus::Working | WorkerStatus::WaitingPermission => {
                    tracing::info!(
                        worker_id = %member.id,
                        status = ?member.status,
                        "suspend: waiting for member to finish"
                    );
                    pending_member_ids.push(member.id.clone());
                }
                _ => {}
            }
        }

        // Phase 3: Poll until all in-progress members finish.
        // Supervisors remain alive so they can approve permission prompts.
        let start = Instant::now();

        while !pending_member_ids.is_empty() && start.elapsed() < SUSPEND_TIMEOUT {
            std::thread::sleep(SUSPEND_POLL_INTERVAL);

            pending_member_ids.retain(|id| {
                let worker = match self.interactor.data_store.find_worker(id) {
                    Ok(Some(w)) => w,
                    _ => return false,
                };

                match worker.status {
                    WorkerStatus::Idle => {
                        tracing::info!(worker_id = %id, "suspend: member finished, suspending");
                        if self.suspend_single_worker(id, &worker.container_id) {
                            suspended_count += 1;
                        }
                        false
                    }
                    WorkerStatus::Crashed => {
                        // Plan: crashed during suspend → mark Suspended without recovery
                        tracing::info!(worker_id = %id, "suspend: crashed member, marking Suspended");
                        if let Err(e) = self
                            .interactor
                            .data_store
                            .update_worker_status(id, WorkerStatus::Suspended)
                        {
                            tracing::error!(worker_id = %id, error = %e, "failed to mark crashed worker as Suspended");
                        } else {
                            suspended_count += 1;
                        }
                        false
                    }
                    WorkerStatus::Working | WorkerStatus::WaitingPermission => true,
                    _ => false,
                }
            });

            if !pending_member_ids.is_empty() {
                tracing::info!(
                    remaining = pending_member_ids.len(),
                    elapsed_secs = start.elapsed().as_secs(),
                    "suspend: still waiting for members to finish"
                );
            }
        }

        // Force-stop members that did not finish within the timeout
        for id in &pending_member_ids {
            let worker = match self.interactor.data_store.find_worker(id) {
                Ok(Some(w)) => w,
                _ => continue,
            };
            tracing::warn!(
                worker_id = %id,
                status = ?worker.status,
                "suspend: timeout exceeded, force-stopping member"
            );
            if self.suspend_single_worker(id, &worker.container_id) {
                suspended_count += 1;
            }
        }

        // Phase 4: All members are done — now suspend supervisors
        for supervisor in &supervisors {
            tracing::info!(worker_id = %supervisor.id, "suspend: suspending supervisor");
            if self.suspend_single_worker(&supervisor.id, &supervisor.container_id) {
                suspended_count += 1;
            }
        }

        // Phase 5: Mark workflows as Suspended
        for workflow_id in &workflow_ids {
            if let Err(e) = self
                .interactor
                .data_store
                .update_workflow_status(workflow_id, WorkflowStatus::Suspended)
            {
                tracing::warn!(
                    workflow_id = %workflow_id,
                    error = %e,
                    "failed to update workflow status to Suspended"
                );
            }
        }

        tracing::info!(suspended_count, "suspend complete");
    }

    /// Stop a single worker's container and mark it as Suspended.
    /// Returns true if successful.
    fn suspend_single_worker(
        &self,
        worker_id: &WorkerId,
        container_id: &palette_domain::worker::ContainerId,
    ) -> bool {
        if let Err(e) = self.interactor.container.stop_container(container_id) {
            tracing::warn!(worker_id = %worker_id, error = %e, "failed to stop container");
            return false;
        }
        if let Err(e) = self
            .interactor
            .data_store
            .update_worker_status(worker_id, WorkerStatus::Suspended)
        {
            tracing::warn!(worker_id = %worker_id, error = %e, "failed to update status to Suspended");
            return false;
        }
        true
    }
}
