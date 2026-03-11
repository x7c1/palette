use super::Orchestrator;
use palette_docker::WorkspaceVolume;
use palette_domain::agent::AgentId;
use palette_domain::job::{Job, JobId, JobStatus, JobType};
use palette_domain::rule::{RuleEffect, RuleEngine};
use palette_domain::server::{PendingDelivery, PersistentState};

impl Orchestrator {
    /// Processes rule engine effects: auto-assign jobs, spawn/destroy members.
    /// Returns a list of messages that need to be sent to members via tmux.
    ///
    /// The caller is responsible for saving state after this function returns.
    pub(super) fn process_effects(
        &self,
        effects: &[RuleEffect],
        infra: &mut PersistentState,
    ) -> crate::Result<Vec<PendingDelivery>> {
        let mut deliveries = Vec::new();
        let mut pending: Vec<RuleEffect> = effects.to_vec();

        while let Some(effect) = pending.pop() {
            match &effect {
                RuleEffect::AutoAssign { job_id } => {
                    self.handle_auto_assign(job_id, infra, &mut deliveries)?;
                }
                RuleEffect::DestroyMember { member_id } => {
                    self.handle_destroy_member(member_id, infra);
                }
                RuleEffect::StatusChanged { job_id, new_status } => {
                    let chained = self.handle_status_changed(job_id, *new_status)?;
                    pending.extend(chained);
                }
                _ => {}
            }
        }

        Ok(deliveries)
    }

    fn handle_auto_assign(
        &self,
        job_id: &JobId,
        infra: &mut PersistentState,
        deliveries: &mut Vec<PendingDelivery>,
    ) -> crate::Result<()> {
        // Only assign if the job is truly assignable (ready + all deps done)
        let assignable_jobs = self.db.find_assignable_jobs()?;
        let job = match assignable_jobs.iter().find(|j| j.id == *job_id) {
            Some(j) => j.clone(),
            None => return Ok(()),
        };
        let active = self.db.count_active_members()?;
        if active >= self.docker_config.max_members {
            tracing::info!(
                job_id = %job_id,
                active = active,
                max = self.docker_config.max_members,
                "max members reached, job waits"
            );
            return Ok(());
        }

        // Determine workspace volume based on job type
        let workspace = self.resolve_workspace(job_id, job.job_type)?;

        // Spawn a new member with supervisor_id based on job type
        let member_id = infra.next_member_id();
        let member = self.spawn_member(&member_id, job.job_type, infra, workspace)?;
        let terminal_target = member.terminal_target.clone();
        infra.members.push(member);

        // Assign job
        self.db.assign_job(job_id, &member_id)?;
        tracing::info!(
            job_id = %job_id,
            member_id = %member_id,
            "auto-assigned job"
        );

        // Build job instruction message
        let instruction = format_job_instruction(&job);
        self.db.enqueue_message(&member_id, &instruction)?;

        deliveries.push(PendingDelivery {
            target_id: member_id,
            terminal_target,
        });

        infra.touch();
        Ok(())
    }

    fn handle_destroy_member(&self, member_id: &AgentId, infra: &mut PersistentState) {
        if let Some(member) = infra.remove_member(member_id) {
            tracing::info!(member_id = %member_id, "destroying member container");
            let _ = self.docker.stop_container(&member.container_id);
            let _ = self.docker.remove_container(&member.container_id);
            infra.touch();
        }
    }

    fn resolve_workspace(
        &self,
        job_id: &JobId,
        job_type: JobType,
    ) -> crate::Result<Option<WorkspaceVolume>> {
        match job_type {
            JobType::Craft => Ok(Some(WorkspaceVolume {
                name: format!("palette-workspace-{job_id}"),
                read_only: false,
            })),
            JobType::Review => {
                let crafts = self.db.find_crafts_for_review(job_id)?;
                Ok(crafts.first().map(|w| WorkspaceVolume {
                    name: format!("palette-workspace-{}", w.id),
                    read_only: true,
                }))
            }
        }
    }

    fn handle_status_changed(
        &self,
        job_id: &JobId,
        new_status: JobStatus,
    ) -> crate::Result<Vec<RuleEffect>> {
        let rules = RuleEngine::new(&*self.db, 0); // max_review_rounds unused for status changes
        let chained = rules.on_status_change(job_id, new_status)?;
        for e in &chained {
            tracing::info!(?e, "chained rule engine effect");
        }
        Ok(chained)
    }
}

/// Format a job into an instruction message for a member.
fn format_job_instruction(job: &Job) -> String {
    let mut msg = format!("## Task: {}\n\nID: {}\n", job.title, job.id);
    if let Some(ref desc) = job.description {
        msg.push_str(&format!("\n{desc}\n"));
    }
    if let Some(ref repos) = job.repositories {
        msg.push('\n');
        for repo in repos {
            if let Some(ref branch) = repo.branch {
                msg.push_str(&format!("- {} (branch: {branch})\n", repo.name));
            } else {
                msg.push_str(&format!("- {}\n", repo.name));
            }
        }
    }
    msg.push_str("\nPlease begin working on this task.");
    msg
}

#[cfg(test)]
mod tests {
    use palette_db::Database;
    use palette_domain::agent::*;
    use palette_domain::review::*;
    use palette_domain::rule::*;

    use palette_domain::job::*;

    fn setup_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn jid(s: &str) -> JobId {
        JobId::new(s)
    }

    fn create_craft_review_pair(db: &Database) {
        db.create_job(&CreateJobRequest {
            id: Some(jid("W-001")),
            job_type: JobType::Craft,
            title: "Work".to_string(),
            description: None,
            assignee: Some(AgentId::new("member-a")),
            priority: None,
            repositories: None,
            depends_on: vec![],
        })
        .unwrap();

        db.create_job(&CreateJobRequest {
            id: Some(jid("R-001")),
            job_type: JobType::Review,
            title: "Review".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![jid("W-001")],
        })
        .unwrap();
    }

    #[test]
    fn craft_in_review_enables_reviews() {
        let db = setup_db();
        create_craft_review_pair(&db);

        db.update_job_status(&jid("W-001"), JobStatus::Ready)
            .unwrap();
        db.update_job_status(&jid("W-001"), JobStatus::InProgress)
            .unwrap();
        db.update_job_status(&jid("W-001"), JobStatus::InReview)
            .unwrap();

        let engine = RuleEngine::new(&db, 5);
        let effects = engine
            .on_status_change(&jid("W-001"), JobStatus::InReview)
            .unwrap();

        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            RuleEffect::StatusChanged {
                job_id: jid("R-001"),
                new_status: JobStatus::Todo,
            }
        );
    }

    #[test]
    fn review_todo_triggers_auto_assign() {
        let db = setup_db();
        create_craft_review_pair(&db);

        db.update_job_status(&jid("W-001"), JobStatus::Ready)
            .unwrap();
        db.update_job_status(&jid("W-001"), JobStatus::InProgress)
            .unwrap();
        db.update_job_status(&jid("W-001"), JobStatus::InReview)
            .unwrap();
        db.update_job_status(&jid("R-001"), JobStatus::Todo)
            .unwrap();

        let engine = RuleEngine::new(&db, 5);
        let effects = engine
            .on_status_change(&jid("R-001"), JobStatus::Todo)
            .unwrap();

        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            RuleEffect::AutoAssign {
                job_id: jid("R-001"),
            }
        );
    }

    #[test]
    fn review_auto_assign_chains_from_craft_in_review() {
        // Verify the full chain: craft -> in_review -> review -> todo -> auto_assign
        let db = setup_db();
        create_craft_review_pair(&db);

        db.update_job_status(&jid("W-001"), JobStatus::Ready)
            .unwrap();
        db.update_job_status(&jid("W-001"), JobStatus::InProgress)
            .unwrap();
        db.update_job_status(&jid("W-001"), JobStatus::InReview)
            .unwrap();

        let engine = RuleEngine::new(&db, 5);

        // Step 1: craft -> in_review produces StatusChanged for review
        let effects = engine
            .on_status_change(&jid("W-001"), JobStatus::InReview)
            .unwrap();
        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            RuleEffect::StatusChanged {
                job_id: jid("R-001"),
                new_status: JobStatus::Todo,
            }
        );

        // Step 2: chained StatusChanged(R-001, Todo) produces AutoAssign
        let chained = engine
            .on_status_change(&jid("R-001"), JobStatus::Todo)
            .unwrap();
        assert_eq!(chained.len(), 1);
        assert_eq!(
            chained[0],
            RuleEffect::AutoAssign {
                job_id: jid("R-001"),
            }
        );
    }

    #[test]
    fn changes_requested_reverts_craft() {
        let db = setup_db();
        create_craft_review_pair(&db);

        db.update_job_status(&jid("W-001"), JobStatus::Ready)
            .unwrap();
        db.update_job_status(&jid("W-001"), JobStatus::InProgress)
            .unwrap();
        db.update_job_status(&jid("W-001"), JobStatus::InReview)
            .unwrap();
        db.update_job_status(&jid("R-001"), JobStatus::InProgress)
            .unwrap();

        let sub = db
            .submit_review(
                &jid("R-001"),
                &SubmitReviewRequest {
                    verdict: Verdict::ChangesRequested,
                    summary: None,
                    comments: vec![],
                },
            )
            .unwrap();

        let engine = RuleEngine::new(&db, 5);
        let effects = engine.on_review_submitted(&jid("R-001"), &sub).unwrap();

        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            RuleEffect::StatusChanged {
                job_id: jid("W-001"),
                new_status: JobStatus::InProgress,
            }
        );

        let craft = db.get_job(&jid("W-001")).unwrap().unwrap();
        assert_eq!(craft.status, JobStatus::InProgress);
    }

    #[test]
    fn approved_completes_craft() {
        let db = setup_db();
        create_craft_review_pair(&db);

        db.update_job_status(&jid("W-001"), JobStatus::Ready)
            .unwrap();
        db.update_job_status(&jid("W-001"), JobStatus::InProgress)
            .unwrap();
        db.update_job_status(&jid("W-001"), JobStatus::InReview)
            .unwrap();
        db.update_job_status(&jid("R-001"), JobStatus::InProgress)
            .unwrap();

        let sub = db
            .submit_review(
                &jid("R-001"),
                &SubmitReviewRequest {
                    verdict: Verdict::Approved,
                    summary: Some("LGTM".to_string()),
                    comments: vec![],
                },
            )
            .unwrap();

        let engine = RuleEngine::new(&db, 5);
        let effects = engine.on_review_submitted(&jid("R-001"), &sub).unwrap();

        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            RuleEffect::StatusChanged {
                job_id: jid("W-001"),
                new_status: JobStatus::Done,
            }
        );
    }

    #[test]
    fn escalation_on_max_rounds() {
        let db = setup_db();
        create_craft_review_pair(&db);

        db.update_job_status(&jid("W-001"), JobStatus::Ready)
            .unwrap();
        db.update_job_status(&jid("W-001"), JobStatus::InProgress)
            .unwrap();
        db.update_job_status(&jid("W-001"), JobStatus::InReview)
            .unwrap();
        db.update_job_status(&jid("R-001"), JobStatus::InProgress)
            .unwrap();

        // Round 1
        let sub1 = db
            .submit_review(
                &jid("R-001"),
                &SubmitReviewRequest {
                    verdict: Verdict::ChangesRequested,
                    summary: None,
                    comments: vec![],
                },
            )
            .unwrap();
        let engine = RuleEngine::new(&db, 2);
        engine.on_review_submitted(&jid("R-001"), &sub1).unwrap();

        // Reset craft to in_review for round 2
        db.update_job_status(&jid("W-001"), JobStatus::InReview)
            .unwrap();

        // Round 2 - should escalate
        let sub2 = db
            .submit_review(
                &jid("R-001"),
                &SubmitReviewRequest {
                    verdict: Verdict::ChangesRequested,
                    summary: None,
                    comments: vec![],
                },
            )
            .unwrap();
        let engine = RuleEngine::new(&db, 2);
        let effects = engine.on_review_submitted(&jid("R-001"), &sub2).unwrap();

        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], RuleEffect::Escalated { .. }));
    }
}
