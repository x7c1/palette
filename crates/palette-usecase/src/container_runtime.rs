use palette_domain::worker::{ContainerId, WorkerRole, WorkerSessionId};
use std::path::Path;

/// Port for container lifecycle management.
///
/// Abstracts Docker operations so that the orchestrator and server
/// can be tested with mock implementations.
pub trait ContainerRuntime: Send + Sync {
    fn create_container(
        &self,
        name: &str,
        image: &str,
        role: WorkerRole,
        session_name: &str,
        mounts: ContainerMounts,
    ) -> Result<ContainerId, Box<dyn std::error::Error + Send + Sync>>;

    fn start_container(
        &self,
        container_id: &ContainerId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn stop_container(
        &self,
        container_id: &ContainerId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn remove_container(
        &self,
        container_id: &ContainerId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn is_container_running(&self, container_id: &str) -> bool;

    /// Check whether a Claude Code process is running inside the container.
    fn is_claude_running(&self, container_id: &ContainerId) -> bool;

    fn list_managed_containers(
        &self,
    ) -> Result<Vec<ContainerId>, Box<dyn std::error::Error + Send + Sync>>;

    fn write_settings(
        &self,
        container_id: &ContainerId,
        template_path: &Path,
        worker_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn copy_file_to_container(
        &self,
        container_id: &ContainerId,
        local_path: &Path,
        container_path: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn copy_dir_to_container(
        &self,
        container_id: &ContainerId,
        local_dir: &Path,
        container_path: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn read_container_file(
        &self,
        container_id: &ContainerId,
        path: &str,
        tail_lines: usize,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>>;

    fn claude_exec_command(
        &self,
        container_id: &ContainerId,
        prompt_file: &str,
        role: WorkerRole,
    ) -> String;

    fn claude_resume_command(
        &self,
        container_id: &ContainerId,
        session_id: &WorkerSessionId,
        role: WorkerRole,
    ) -> String;
}

/// All bind mount configurations for a container.
#[derive(Default)]
pub struct ContainerMounts {
    pub workspace: Option<WorkspaceVolume>,
    pub plan_dir: Option<PlanDirMount>,
    pub artifacts_dir: Option<ArtifactsMount>,
}

/// Workspace bind mount configuration for container creation.
///
/// Each workspace is a `git clone --shared` of a bare cache on the host.
/// The container sees:
///   - `/home/agent/workspace`  — the working tree (bind mount of `host_path`)
///   - `/home/agent/repo-cache` — the bare cache (read-only bind mount of `repo_cache_path`)
pub struct WorkspaceVolume {
    /// Absolute path on the host to the workspace directory
    /// (e.g., "data/workspace/{job_id}").
    pub host_path: String,
    /// Absolute path on the host to the bare repository cache
    /// (e.g., "data/repos/{org}/{repo}.git").
    pub repo_cache_path: String,
    /// If true, mount workspace as read-only (for reviewers).
    pub read_only: bool,
}

/// Plan directory mount configuration for container creation.
pub struct PlanDirMount {
    pub host_path: String,
    pub read_only: bool,
}

/// Artifacts directory bind mount configuration for container creation.
///
/// Mounted at `/home/agent/artifacts` inside the container.
/// Reviewers and Review Integrators write review results here;
/// Crafters read feedback from here.
pub struct ArtifactsMount {
    /// Absolute path on the host to the artifacts directory
    /// (e.g., "data/artifacts/{workflow_id}/{craft_job_id}").
    pub host_path: String,
    /// If true, mount as read-only.
    pub read_only: bool,
}
