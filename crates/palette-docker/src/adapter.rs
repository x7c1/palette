use crate::{DockerManager, is_container_running, read_container_file};
use palette_domain::worker::{ContainerId, WorkerRole, WorkerSessionId};
use palette_usecase::ContainerRuntime;
use palette_usecase::container_runtime::ContainerMounts;
use std::path::Path;

impl ContainerRuntime for DockerManager {
    fn create_container(
        &self,
        name: &str,
        image: &str,
        role: WorkerRole,
        session_name: &str,
        mounts: ContainerMounts,
    ) -> Result<ContainerId, Box<dyn std::error::Error + Send + Sync>> {
        let ws = mounts.workspace.map(|w| crate::WorkspaceVolume {
            host_path: w.host_path,
            repo_cache_path: w.repo_cache_path,
            read_only: w.read_only,
        });
        let pd = mounts.plan_dir.map(|p| crate::PlanDirMount {
            host_path: p.host_path,
            read_only: p.read_only,
        });
        let ad = mounts.artifacts_dir.map(|a| crate::ArtifactsMount {
            host_path: a.host_path,
            read_only: a.read_only,
        });
        Ok(self.create_container(name, image, role, session_name, ws, pd, ad)?)
    }

    fn start_container(
        &self,
        container_id: &ContainerId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.start_container(container_id)?)
    }

    fn stop_container(
        &self,
        container_id: &ContainerId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.stop_container(container_id)?)
    }

    fn remove_container(
        &self,
        container_id: &ContainerId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.remove_container(container_id)?)
    }

    fn is_container_running(&self, container_id: &str) -> bool {
        is_container_running(container_id)
    }

    fn is_claude_running(&self, container_id: &ContainerId) -> bool {
        DockerManager::is_claude_running(container_id)
    }

    fn list_managed_containers(
        &self,
    ) -> Result<Vec<ContainerId>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.list_managed_containers()?)
    }

    fn write_settings(
        &self,
        container_id: &ContainerId,
        template_path: &Path,
        worker_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.write_settings(container_id, template_path, worker_id)?)
    }

    fn copy_file_to_container(
        &self,
        container_id: &ContainerId,
        local_path: &Path,
        container_path: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(DockerManager::copy_file_to_container(
            container_id,
            local_path,
            container_path,
        )?)
    }

    fn copy_dir_to_container(
        &self,
        container_id: &ContainerId,
        local_dir: &Path,
        container_path: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(DockerManager::copy_dir_to_container(
            container_id,
            local_dir,
            container_path,
        )?)
    }

    fn read_container_file(
        &self,
        container_id: &ContainerId,
        path: &str,
        tail_lines: usize,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Ok(read_container_file(container_id, path, tail_lines)?)
    }

    fn claude_exec_command(
        &self,
        container_id: &ContainerId,
        prompt_file: &str,
        role: WorkerRole,
        workdir: Option<&str>,
    ) -> String {
        DockerManager::claude_exec_command(container_id, prompt_file, role, workdir)
    }

    fn claude_resume_command(
        &self,
        container_id: &ContainerId,
        session_id: &WorkerSessionId,
        role: WorkerRole,
        workdir: Option<&str>,
    ) -> String {
        DockerManager::claude_resume_command(container_id, session_id, role, workdir)
    }
}
