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
        workspace: Option<WorkspaceVolume>,
        plan_dir: Option<PlanDirMount>,
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

    fn list_managed_containers(
        &self,
    ) -> Result<Vec<ContainerId>, Box<dyn std::error::Error + Send + Sync>>;

    fn write_settings(
        &self,
        container_id: &ContainerId,
        template_path: &Path,
        member_id: &str,
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

/// Workspace volume configuration for container creation.
pub struct WorkspaceVolume {
    pub name: String,
    pub read_only: bool,
}

/// Plan directory mount configuration for container creation.
pub struct PlanDirMount {
    pub host_path: String,
    pub read_only: bool,
}
