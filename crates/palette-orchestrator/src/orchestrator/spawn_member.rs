use super::Orchestrator;
use palette_docker::DockerManager;
use palette_domain::agent::{AgentId, AgentRole, AgentState, AgentStatus};
use palette_domain::server::PersistentState;

impl Orchestrator {
    pub(super) fn spawn_member(
        &self,
        member_id: &AgentId,
        infra: &PersistentState,
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
        let terminal_target = self.tmux.create_pane(leader_target)?;

        let member_id_str = member_id.as_ref();
        let container_id = self.docker.create_container(
            member_id_str,
            &self.docker_config.member_image,
            AgentRole::Member,
            session_name,
        )?;
        self.docker.start_container(&container_id)?;
        self.docker.write_settings(
            &container_id,
            std::path::Path::new(&self.docker_config.settings_template),
            member_id_str,
        )?;
        DockerManager::copy_file_to_container(
            &container_id,
            std::path::Path::new(&self.docker_config.member_prompt),
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
        self.tmux.send_keys(&terminal_target, &cmd)?;
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
}
