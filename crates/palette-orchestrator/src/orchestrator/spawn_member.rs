use super::Orchestrator;
use palette_docker::{DockerManager, PlanDirMount, WorkspaceVolume};
use palette_domain::agent::{AgentId, AgentRole, AgentState, AgentStatus};
use palette_domain::job::JobType;
use palette_domain::server::PersistentState;

impl Orchestrator {
    pub(super) fn spawn_member(
        &self,
        member_id: &AgentId,
        job_type: JobType,
        supervisor_id: &AgentId,
        infra: &PersistentState,
        workspace: Option<WorkspaceVolume>,
    ) -> crate::Result<AgentState> {
        let session_name = &infra.session_name;
        let supervisor_id = supervisor_id.clone();

        // Create a new tmux pane by splitting from the assigned supervisor's pane
        let supervisor_state = infra
            .find_supervisor(&supervisor_id)
            .or_else(|| infra.supervisors.first())
            .ok_or_else(|| {
                crate::Error::Internal(
                    "no supervisor found; cannot spawn member without a supervisor pane".into(),
                )
            })?;
        let terminal_target = self.tmux.create_pane(&supervisor_state.terminal_target)?;

        let member_id_str = member_id.as_ref();
        let plan_dir_abs = std::fs::canonicalize(&self.plan_dir)
            .map_err(|e| crate::Error::Internal(format!("failed to resolve plan_dir: {e}")))?;
        let plan_dir_mount = PlanDirMount {
            host_path: plan_dir_abs.to_string_lossy().to_string(),
            read_only: job_type == JobType::Review,
        };

        let container_id = self.docker.create_container(
            member_id_str,
            &self.docker_config.member_image,
            AgentRole::Member,
            session_name,
            workspace,
            Some(plan_dir_mount),
        )?;
        self.docker.start_container(&container_id)?;
        self.docker.write_settings(
            &container_id,
            std::path::Path::new(&self.docker_config.settings_template),
            member_id_str,
        )?;
        let prompt_path = match job_type {
            JobType::Craft => &self.docker_config.crafter_prompt,
            JobType::Review => &self.docker_config.reviewer_prompt,
        };
        DockerManager::copy_file_to_container(
            &container_id,
            std::path::Path::new(prompt_path),
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
            supervisor_id,
            container_id,
            terminal_target,
            status: AgentStatus::Booting,
            session_id: None,
            task_id: palette_domain::task::TaskId::new(""),
        })
    }
}
