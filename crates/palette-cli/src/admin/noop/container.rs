use super::{BoxErr, unsupported};
use palette_domain::worker::{ContainerId, WorkerRole, WorkerSessionId};
use palette_usecase::{ContainerMounts, ContainerRuntime};
use std::path::Path;

pub(in crate::admin) struct NoopContainer;

impl ContainerRuntime for NoopContainer {
    fn create_container(
        &self,
        _name: &str,
        _image: &str,
        _role: WorkerRole,
        _session_name: &str,
        _mounts: ContainerMounts,
    ) -> Result<ContainerId, BoxErr> {
        unsupported("create_container")
    }

    fn start_container(&self, _container_id: &ContainerId) -> Result<(), BoxErr> {
        unsupported("start_container")
    }

    fn stop_container(&self, _container_id: &ContainerId) -> Result<(), BoxErr> {
        unsupported("stop_container")
    }

    fn remove_container(&self, _container_id: &ContainerId) -> Result<(), BoxErr> {
        unsupported("remove_container")
    }

    fn is_container_running(&self, _container_id: &str) -> bool {
        false
    }

    fn is_claude_running(&self, _container_id: &ContainerId) -> bool {
        false
    }

    fn list_managed_containers(&self) -> Result<Vec<ContainerId>, BoxErr> {
        Ok(vec![])
    }

    fn write_settings(
        &self,
        _container_id: &ContainerId,
        _template_path: &Path,
        _worker_id: &str,
    ) -> Result<(), BoxErr> {
        unsupported("write_settings")
    }

    fn copy_file_to_container(
        &self,
        _container_id: &ContainerId,
        _local_path: &Path,
        _container_path: &str,
    ) -> Result<(), BoxErr> {
        unsupported("copy_file_to_container")
    }

    fn copy_dir_to_container(
        &self,
        _container_id: &ContainerId,
        _local_dir: &Path,
        _container_path: &str,
    ) -> Result<(), BoxErr> {
        unsupported("copy_dir_to_container")
    }

    fn read_container_file(
        &self,
        _container_id: &ContainerId,
        _path: &str,
        _tail_lines: usize,
    ) -> Result<String, BoxErr> {
        unsupported("read_container_file")
    }

    fn claude_exec_command(
        &self,
        _container_id: &ContainerId,
        _prompt_file: &str,
        _role: WorkerRole,
        _workdir: Option<&str>,
    ) -> String {
        String::new()
    }

    fn claude_resume_command(
        &self,
        _container_id: &ContainerId,
        _session_id: &WorkerSessionId,
        _role: WorkerRole,
        _workdir: Option<&str>,
    ) -> String {
        String::new()
    }
}
