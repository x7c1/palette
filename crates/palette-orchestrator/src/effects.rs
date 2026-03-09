use crate::DockerConfig;
use palette_db::Database;
use palette_docker::DockerManager;
use palette_domain::{
    AgentId, AgentRole, AgentState, AgentStatus, PendingDelivery, PersistentState, RuleEffect,
    RuleEngine, Task,
};
use palette_tmux::TerminalManager;

/// Processes rule engine effects: auto-assign tasks, spawn/destroy members.
/// Returns a list of messages that need to be sent to members via tmux.
///
/// The caller is responsible for saving state after this function returns.
pub fn process_effects<T: TerminalManager>(
    effects: &[RuleEffect],
    db: &Database,
    infra: &mut PersistentState,
    docker: &DockerManager,
    tmux: &T,
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

/// Delivers queued messages to idle targets.
pub fn deliver_queued_messages<T: TerminalManager>(
    target_id: &AgentId,
    db: &Database,
    infra: &mut PersistentState,
    tmux: &T,
) -> crate::Result<bool> {
    let member = infra
        .find_member(target_id)
        .or_else(|| infra.find_leader(target_id));

    let terminal_target = match member {
        Some(m) if m.status == AgentStatus::Idle => m.terminal_target.clone(),
        _ => return Ok(false),
    };

    if let Some(msg) = db.dequeue_message(target_id)? {
        tmux.send_keys(&terminal_target, &msg.message)?;
        // Update status to Working
        if let Some(m) = infra.find_member_mut(target_id) {
            m.status = AgentStatus::Working;
        } else if let Some(l) = infra.find_leader_mut(target_id) {
            l.status = AgentStatus::Working;
        }
        infra.touch();
        tracing::info!(target_id = %target_id, "delivered queued message");
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Format a task into an instruction message for a member.
fn format_task_instruction(task: &Task) -> String {
    let mut msg = format!("## Task: {}\n\nID: {}\n", task.title, task.id);
    if let Some(ref desc) = task.description {
        msg.push_str(&format!("\n{desc}\n"));
    }
    if let Some(ref repos) = task.repositories {
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

fn spawn_member<T: TerminalManager>(
    member_id: &AgentId,
    infra: &PersistentState,
    docker: &DockerManager,
    tmux: &T,
    config: &DockerConfig,
) -> crate::Result<AgentState> {
    let session_name = &infra.session_name;

    // Create a new tmux pane by splitting from the leader's pane
    let leader_target = infra
        .leaders
        .first()
        .map(|l| &l.terminal_target)
        .ok_or_else(|| {
            crate::Error::Internal(
                "no leader found; cannot spawn member without a leader pane".into(),
            )
        })?;
    let terminal_target = tmux.create_pane(leader_target)?;

    let member_id_str = member_id.as_ref();
    let container_id = docker.create_container(
        member_id_str,
        &config.member_image,
        AgentRole::Member,
        session_name,
    )?;
    docker.start_container(&container_id)?;
    docker.write_settings(
        &container_id,
        std::path::Path::new(&config.settings_template),
        member_id_str,
    )?;
    DockerManager::copy_file_to_container(
        &container_id,
        std::path::Path::new(&config.member_prompt),
        "/home/agent/prompt.md",
    )?;
    DockerManager::copy_dir_to_container(
        &container_id,
        std::path::Path::new("claude-code-plugin"),
        "/home/agent/claude-code-plugin",
    )?;

    let cmd = DockerManager::claude_exec_command(
        &container_id,
        "/home/agent/prompt.md",
        AgentRole::Member,
    );
    tmux.send_keys(&terminal_target, &cmd)?;
    tracing::info!(member_id = %member_id, "spawned member");

    Ok(AgentState {
        id: member_id.clone(),
        role: AgentRole::Member,
        leader_id: AgentId::new("leader-1"),
        container_id,
        terminal_target,
        status: AgentStatus::Booting,
        session_id: None,
    })
}
