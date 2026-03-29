use super::Orchestrator;
use palette_domain::task::TaskId;
use palette_domain::worker::{ContainerId, WorkerId, WorkerRole, WorkerStatus};
use palette_usecase::data_store::InsertWorkerRequest;

impl Orchestrator {
    /// Spawn a dynamic supervisor for a composite task.
    /// Creates a tmux window and Docker container, then registers in DB.
    /// If Docker fails, the supervisor is still registered with an empty container_id.
    pub(super) fn handle_spawn_supervisor(
        &self,
        task_id: &TaskId,
        role: WorkerRole,
    ) -> crate::Result<WorkerId> {
        let task_state = self
            .interactor
            .data_store
            .get_task_state(task_id)?
            .ok_or_else(|| crate::Error::Internal(format!("task not found: {task_id}")))?;
        let seq = self
            .interactor
            .data_store
            .increment_worker_counter(&task_state.workflow_id)?;
        let sup_id = WorkerId::next_supervisor(seq, role);

        // Create a tmux window for this supervisor
        let sup_name = sup_id.as_ref();
        let terminal_target = self.interactor.terminal.create_target(sup_name)?;

        // Select Docker image and prompt based on role
        let (image, prompt_path) = match role {
            WorkerRole::Leader => (
                &self.docker_config.leader_image,
                &self.docker_config.leader_prompt,
            ),
            WorkerRole::ReviewIntegrator => (
                &self.docker_config.review_integrator_image,
                &self.docker_config.review_integrator_prompt,
            ),
            WorkerRole::Member => {
                return Err(crate::Error::Internal(
                    "cannot spawn a supervisor with Member role".into(),
                ));
            }
        };

        let container_id = self.spawn_supervisor_container(
            sup_name,
            image,
            prompt_path,
            &self.session_name,
            &terminal_target,
            role,
        )?;

        // Register in DB
        self.interactor
            .data_store
            .insert_worker(&InsertWorkerRequest {
                id: sup_id.clone(),
                workflow_id: task_state.workflow_id,
                role,
                status: WorkerStatus::Booting,
                supervisor_id: None,
                container_id,
                terminal_target,
                // Claude Code session does not exist yet; it will be created once the process boots.
                session_id: None,
                task_id: task_id.clone(),
            })?;

        tracing::info!(
            supervisor_id = %sup_id,
            task_id = %task_id,
            role = %role,
            "spawned dynamic supervisor"
        );
        Ok(sup_id)
    }

    fn spawn_supervisor_container(
        &self,
        name: &str,
        image: &str,
        prompt_path: &str,
        session_name: &str,
        terminal_target: &palette_domain::terminal::TerminalTarget,
        role: WorkerRole,
    ) -> crate::Result<ContainerId> {
        let container_id = self.interactor.container.create_container(
            name,
            image,
            role,
            session_name,
            None,
            None,
        )?;
        self.interactor.container.start_container(&container_id)?;
        self.interactor.container.write_settings(
            &container_id,
            std::path::Path::new(&self.docker_config.settings_template),
            name,
        )?;
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
            role,
        );
        self.interactor.terminal.send_keys(terminal_target, &cmd)?;

        Ok(container_id)
    }
}
