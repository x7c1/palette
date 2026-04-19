use super::Orchestrator;
use super::plan_location::PlanLocation;
use palette_domain::job::{JobDetail, JobType};
use palette_domain::task::TaskId;
use palette_domain::worker::{WorkerId, WorkerRole, WorkerState, WorkerStatus};
use palette_usecase::{
    ArtifactsMount, ContainerMounts, PerspectiveMount, PlanDirMount, WorkspaceVolume,
};

// Plans are authored by the Operator (via /palette:plan or reconciliation) and
// only read by Members. Mounting them read-only prevents a Member from
// accidentally writing back to the host plan directory.
const PLAN_MOUNT_READ_ONLY: bool = true;

impl Orchestrator {
    /// Spawn a member container. Returns the WorkerState for DB registration.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn spawn_member(
        &self,
        member_id: &WorkerId,
        job_type: JobType,
        job_detail: &JobDetail,
        supervisor_id: &WorkerId,
        task_id: &TaskId,
        workspace: Option<WorkspaceVolume>,
        artifacts_dir: Option<ArtifactsMount>,
        plan_loc: &PlanLocation,
    ) -> crate::Result<WorkerState> {
        let session_name = &self.session_name;
        let supervisor_id = supervisor_id.clone();

        // Look up supervisor from DB to find its pane and workflow
        let supervisor_state = self
            .interactor
            .data_store
            .find_worker(&supervisor_id)?
            .ok_or_else(|| crate::Error::InvalidTaskState {
                task_id: task_id.clone(),
                detail: "no supervisor found; cannot spawn member without a supervisor pane".into(),
            })?;
        let workflow_id = supervisor_state.workflow_id.clone();

        let terminal_target = self
            .interactor
            .terminal
            .create_pane(&supervisor_state.terminal_target)?;

        let member_id_str = member_id.as_ref();
        let has_workspace = workspace.is_some();

        // Only attach the plan mount when the Blueprint is outside the
        // workspace. For Repo-inside-Plan mode the workspace mount already
        // carries the Blueprint (committed on the work branch).
        let plan_dir_mount = plan_loc.plan_dir_host_path().map(|host| PlanDirMount {
            host_path: host.to_string_lossy().to_string(),
            read_only: PLAN_MOUNT_READ_ONLY,
        });

        // Resolve perspective mounts for review jobs
        let perspective_dirs = self.resolve_perspective_mounts(job_detail);

        let container_id = self.interactor.container.create_container(
            member_id_str,
            &self.docker_config.member_image,
            WorkerRole::Member,
            session_name,
            ContainerMounts {
                workspace,
                plan_dir: plan_dir_mount,
                artifacts_dir,
                perspective_dirs,
            },
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
            // ReviewIntegrate, Orchestrator, and Operator don't spawn members
            JobType::ReviewIntegrate | JobType::Orchestrator | JobType::Operator => {
                unreachable!("mechanized job types do not spawn members")
            }
        };
        self.interactor.container.copy_file_to_container(
            &container_id,
            std::path::Path::new(prompt_path),
            "/home/agent/prompt.md",
        )?;
        self.interactor.container.copy_dir_to_container(
            &container_id,
            std::path::Path::new("plugins/worker"),
            "/home/agent/plugins/worker",
        )?;
        self.interactor.container.copy_file_to_container(
            &container_id,
            std::path::Path::new("plugins/worker/hooks/guard-cd-chain.sh"),
            "/home/agent/.claude/hooks/guard-cd-chain.sh",
        )?;

        let workdir = if has_workspace {
            Some("/home/agent/workspace")
        } else {
            None
        };
        let cmd = self.interactor.container.claude_exec_command(
            &container_id,
            "/home/agent/prompt.md",
            WorkerRole::Member,
            workdir,
        );
        self.interactor.terminal.send_keys(&terminal_target, &cmd)?;
        tracing::info!(member_id = %member_id, "spawned member");

        Ok(WorkerState {
            id: member_id.clone(),
            workflow_id,
            role: WorkerRole::Member,
            supervisor_id: Some(supervisor_id),
            container_id,
            terminal_target,
            status: WorkerStatus::Booting,
            // Claude Code session does not exist yet; it will be created once the process boots.
            session_id: None,
            task_id: task_id.clone(),
        })
    }

    /// Resolve perspective mounts for a job's detail.
    ///
    /// Mounts entire base directories rather than individual paths, so
    /// relative links between perspective documents resolve correctly.
    /// Returns an empty vec if the job has no perspective.
    fn resolve_perspective_mounts(&self, job_detail: &JobDetail) -> Vec<PerspectiveMount> {
        let Some(perspective_name) = job_detail.perspective() else {
            return vec![];
        };
        let Some(perspective) = self.perspectives.find(perspective_name.as_ref()) else {
            return vec![];
        };

        // Collect unique dir_names used by this perspective
        let mut seen = std::collections::HashSet::new();
        perspective
            .paths
            .iter()
            .filter(|pp| seen.insert(pp.dir_name.clone()))
            .filter_map(|pp| {
                let base_dir = self.perspectives.dirs.get(&pp.dir_name)?;
                let container_path = format!("/home/agent/perspective/{}", pp.dir_name);
                Some(PerspectiveMount {
                    host_path: base_dir.to_string_lossy().to_string(),
                    container_path,
                })
            })
            .collect()
    }
}
