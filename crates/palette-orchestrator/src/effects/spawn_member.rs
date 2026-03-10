use crate::DockerConfig;
use palette_docker::DockerManager;
use palette_domain::{AgentId, AgentRole, AgentState, AgentStatus, PersistentState};
use palette_tmux::TmuxManager;

pub(super) fn spawn_member(
    member_id: &AgentId,
    infra: &PersistentState,
    docker: &DockerManager,
    tmux: &TmuxManager,
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
