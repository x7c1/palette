use super::{format_task_instruction, spawn_member};
use crate::DockerConfig;
use palette_db::Database;
use palette_docker::DockerManager;
use palette_domain::{PendingDelivery, PersistentState, RuleEffect, RuleEngine};
use palette_tmux::TmuxManager;

/// Processes rule engine effects: auto-assign tasks, spawn/destroy members.
/// Returns a list of messages that need to be sent to members via tmux.
///
/// The caller is responsible for saving state after this function returns.
pub fn process_effects(
    effects: &[RuleEffect],
    db: &Database,
    infra: &mut PersistentState,
    docker: &DockerManager,
    tmux: &TmuxManager,
    config: &DockerConfig,
) -> crate::Result<Vec<PendingDelivery>> {
    let mut deliveries = Vec::new();
    let mut pending: Vec<RuleEffect> = effects.to_vec();

    while let Some(effect) = pending.pop() {
        match &effect {
            RuleEffect::AutoAssign { task_id } => {
                // Only assign if the task is truly assignable (ready + all deps done)
                let assignable = db.find_assignable_tasks()?;
                let task = match assignable.iter().find(|t| t.id == *task_id) {
                    Some(t) => t.clone(),
                    None => continue,
                };
                let active = db.count_active_members()?;
                if active >= config.max_members {
                    tracing::info!(
                        task_id = %task_id,
                        active = active,
                        max = config.max_members,
                        "max members reached, task waits"
                    );
                    continue;
                }
                // Spawn a new member
                let member_id = infra.next_member_id();
                let member = spawn_member(&member_id, infra, docker, tmux, config)?;
                let terminal_target = member.terminal_target.clone();
                infra.members.push(member);

                // Assign task
                db.assign_task(task_id, &member_id)?;
                tracing::info!(
                    task_id = %task_id,
                    member_id = %member_id,
                    "auto-assigned task"
                );

                // Build task instruction message
                let instruction = format_task_instruction(&task);
                db.enqueue_message(&member_id, &instruction)?;

                deliveries.push(PendingDelivery {
                    target_id: member_id,
                    terminal_target,
                });

                infra.touch();
            }
            RuleEffect::DestroyMember { member_id } => {
                if let Some(member) = infra.remove_member(member_id) {
                    tracing::info!(member_id = %member_id, "destroying member container");
                    let _ = docker.stop_container(&member.container_id);
                    let _ = docker.remove_container(&member.container_id);
                    infra.touch();
                }
            }
            RuleEffect::StatusChanged {
                task_id,
                new_status,
            } => {
                // Chain: re-evaluate rules for the new status
                let rules = RuleEngine::new(0); // max_review_rounds unused for status changes
                let chained = rules.on_status_change(db, task_id, *new_status)?;
                for e in &chained {
                    tracing::info!(?e, "chained rule engine effect");
                }
                pending.extend(chained);
            }
            _ => {}
        }
    }

    Ok(deliveries)
}
