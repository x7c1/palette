mod activate_review;
mod assign_new_job;
mod complete_job;
mod destroy_member;
mod job_instruction;
mod reactivate_member;
mod review_verdict;

use std::collections::VecDeque;

use super::Orchestrator;
use palette_domain::rule::RuleEffect;
use palette_domain::server::PendingDelivery;
use palette_domain::worker::WorkerId;

pub(super) struct ProcessEffectsResult {
    pub deliveries: Vec<PendingDelivery>,
    pub spawned_supervisors: Vec<WorkerId>,
}

impl Orchestrator {
    /// Processes rule engine effects: auto-assign jobs, spawn/destroy members/supervisors.
    /// Returns a list of messages that need to be sent to members via tmux.
    ///
    /// Effects are processed in FIFO order so that SpawnSupervisor is handled
    /// before AssignNewJob for child tasks.
    pub(super) fn process_effects(
        &self,
        effects: &[RuleEffect],
    ) -> crate::Result<ProcessEffectsResult> {
        let mut deliveries = Vec::new();
        let mut spawned_supervisors = Vec::new();
        let mut pending: VecDeque<RuleEffect> = effects.iter().cloned().collect();

        while let Some(effect) = pending.pop_front() {
            let chained = match &effect {
                RuleEffect::AssignNewJob { job_id } => {
                    self.assign_new_job(job_id, &mut deliveries)?;
                    vec![]
                }
                RuleEffect::ReactivateMember { job_id, member_id } => {
                    self.reactivate_member(job_id, member_id, &mut deliveries)?;
                    vec![]
                }
                RuleEffect::DestroyMember { member_id } => {
                    self.destroy_member(member_id);
                    vec![]
                }
                RuleEffect::CraftReadyForReview { craft_job_id } => {
                    self.activate_child_review_tasks(craft_job_id)?
                }
                RuleEffect::ReviewVerdict {
                    review_job_id,
                    verdict,
                } => self.handle_review_verdict(review_job_id, *verdict)?,
                RuleEffect::JobCompleted { job_id } => self.complete_job(job_id)?,
                RuleEffect::SpawnSupervisor { task_id, role } => {
                    match self.handle_spawn_supervisor(task_id, *role) {
                        Ok(sup_id) => spawned_supervisors.push(sup_id),
                        Err(e) => {
                            tracing::error!(error = %e, task_id = %task_id, "failed to spawn supervisor");
                        }
                    }
                    vec![]
                }
                RuleEffect::DestroySupervisor { supervisor_id } => {
                    self.destroy_supervisor(supervisor_id);
                    vec![]
                }
            };
            pending.extend(chained);
        }

        Ok(ProcessEffectsResult {
            deliveries,
            spawned_supervisors,
        })
    }

    fn destroy_supervisor(&self, supervisor_id: &WorkerId) {
        let worker = match self.db.remove_worker(supervisor_id) {
            Ok(Some(w)) => w,
            Ok(None) => return,
            Err(e) => {
                tracing::error!(supervisor_id = %supervisor_id, error = %e, "failed to remove supervisor from DB");
                return;
            }
        };
        tracing::info!(supervisor_id = %supervisor_id, task_id = %worker.task_id, "destroying supervisor");
        if let Err(e) = self.docker.stop_container(&worker.container_id) {
            tracing::warn!(supervisor_id = %supervisor_id, error = %e, "failed to stop supervisor container");
        }
        if let Err(e) = self.docker.remove_container(&worker.container_id) {
            tracing::warn!(supervisor_id = %supervisor_id, error = %e, "failed to remove supervisor container");
        }
    }
}
