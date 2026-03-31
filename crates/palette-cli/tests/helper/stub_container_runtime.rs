use palette_domain::worker::{ContainerId, WorkerRole, WorkerSessionId};
use palette_usecase::container_runtime::ContainerMounts;

/// Stub container runtime for integration tests.
/// All container operations are no-ops; `is_claude_running` always returns
/// true so message delivery proceeds without a real Docker container.
pub struct StubContainerRuntime;

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

impl palette_usecase::ContainerRuntime for StubContainerRuntime {
    fn create_container(
        &self,
        _: &str,
        _: &str,
        _: WorkerRole,
        _: &str,
        _: ContainerMounts,
    ) -> Result<ContainerId, BoxErr> {
        Ok(ContainerId::new("stub"))
    }
    fn start_container(&self, _: &ContainerId) -> Result<(), BoxErr> {
        Ok(())
    }
    fn stop_container(&self, _: &ContainerId) -> Result<(), BoxErr> {
        Ok(())
    }
    fn remove_container(&self, _: &ContainerId) -> Result<(), BoxErr> {
        Ok(())
    }
    fn is_container_running(&self, _: &str) -> bool {
        true
    }
    fn is_claude_running(&self, _: &ContainerId) -> bool {
        true
    }
    fn list_managed_containers(&self) -> Result<Vec<ContainerId>, BoxErr> {
        Ok(vec![])
    }
    fn write_settings(&self, _: &ContainerId, _: &std::path::Path, _: &str) -> Result<(), BoxErr> {
        Ok(())
    }
    fn copy_file_to_container(
        &self,
        _: &ContainerId,
        _: &std::path::Path,
        _: &str,
    ) -> Result<(), BoxErr> {
        Ok(())
    }
    fn copy_dir_to_container(
        &self,
        _: &ContainerId,
        _: &std::path::Path,
        _: &str,
    ) -> Result<(), BoxErr> {
        Ok(())
    }
    fn read_container_file(&self, _: &ContainerId, _: &str, _: usize) -> Result<String, BoxErr> {
        Ok(String::new())
    }
    fn claude_exec_command(&self, _: &ContainerId, _: &str, _: WorkerRole) -> String {
        String::new()
    }
    fn claude_resume_command(&self, _: &ContainerId, _: &WorkerSessionId, _: WorkerRole) -> String {
        String::new()
    }
}
