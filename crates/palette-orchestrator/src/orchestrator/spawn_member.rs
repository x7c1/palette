use super::Orchestrator;
use palette_domain::job::JobType;
use palette_domain::task::TaskId;
use palette_domain::worker::{WorkerId, WorkerRole, WorkerState, WorkerStatus};
use palette_usecase::container_runtime::{PlanDirMount, WorkspaceVolume};

impl Orchestrator {
    /// Spawn a member container. Returns the WorkerState for DB registration.
    pub(super) fn spawn_member(
        &self,
        member_id: &WorkerId,
        job_type: JobType,
        supervisor_id: &WorkerId,
        task_id: &TaskId,
        workspace: Option<WorkspaceVolume>,
    ) -> crate::Result<WorkerState> {
        let session_name = &self.session_name;
        let supervisor_id = supervisor_id.clone();

        // Look up supervisor from DB to find its pane and workflow
        let supervisor_state = self
            .interactor
            .data_store
            .find_worker(&supervisor_id)?
            .ok_or_else(|| {
                crate::Error::Internal(
                    "no supervisor found; cannot spawn member without a supervisor pane".into(),
                )
            })?;
        let workflow_id = supervisor_state.workflow_id.clone();

        let terminal_target = self
            .interactor
            .terminal
            .create_pane(&supervisor_state.terminal_target)?;

        let member_id_str = member_id.as_ref();
        let plan_dir_abs = std::fs::canonicalize(&self.plan_dir)
            .map_err(|e| crate::Error::Internal(format!("failed to resolve plan_dir: {e}")))?;
        let plan_dir_mount = PlanDirMount {
            host_path: plan_dir_abs.to_string_lossy().to_string(),
            read_only: job_type == JobType::Review,
        };

        let container_id = self.interactor.container.create_container(
            member_id_str,
            &self.docker_config.member_image,
            WorkerRole::Member,
            session_name,
            workspace,
            Some(plan_dir_mount),
        )?;
        self.interactor.container.start_container(&container_id)?;
        self.interactor.container.write_settings(
            &container_id,
            std::path::Path::new(&self.docker_config.settings_template),
            member_id_str,
        )?;
        let prompt_path = match job_type {
            JobType::Craft => &self.docker_config.crafter_prompt,
            JobType::Review => &self.docker_config.reviewer_prompt,
        };
        self.interactor.container.copy_file_to_container(
            &container_id,
            std::path::Path::new(prompt_path),
            "/home/agent/prompt.md",
        )?;
        self.interactor.container.copy_dir_to_container(
            &container_id,
            std::path::Path::new("claude-code-plugin"),
            "/home/agent/claude-code-plugin",
        )?;

        let cmd = self.interactor.container.claude_exec_command(
            &container_id,
            "/home/agent/prompt.md",
            WorkerRole::Member,
        );
        self.interactor.terminal.send_keys(&terminal_target, &cmd)?;
        tracing::info!(member_id = %member_id, "spawned member");

        Ok(WorkerState {
            id: member_id.clone(),
            workflow_id,
            role: WorkerRole::Member,
            supervisor_id,
            container_id,
            terminal_target,
            status: WorkerStatus::Booting,
            // Claude Code session does not exist yet; it will be created once the process boots.
            session_id: None,
            task_id: task_id.clone(),
        })
    }
}
