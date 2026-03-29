use palette_domain::worker::{ContainerId, WorkerRole, WorkerSessionId};
use palette_usecase::ContainerRuntime;
use palette_usecase::container_runtime::{PlanDirMount, WorkspaceVolume};
use std::path::Path;
use std::sync::Mutex;

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

pub struct MockContainerRuntime {
    pub running_containers: Mutex<std::collections::HashSet<String>>,
    pub started_containers: Mutex<Vec<ContainerId>>,
}

impl MockContainerRuntime {
    pub fn new() -> Self {
        Self {
            running_containers: Mutex::new(std::collections::HashSet::new()),
            started_containers: Mutex::new(Vec::new()),
        }
    }

    pub fn with_running(container_ids: &[&str]) -> Self {
        let mock = Self::new();
        {
            let mut set = mock.running_containers.lock().unwrap();
            for id in container_ids {
                set.insert(id.to_string());
            }
        }
        mock
    }
}

impl ContainerRuntime for MockContainerRuntime {
    fn is_container_running(&self, container_id: &str) -> bool {
        self.running_containers
            .lock()
            .unwrap()
            .contains(container_id)
    }

    fn is_claude_running(&self, _container_id: &ContainerId) -> bool {
        true
    }

    fn start_container(&self, container_id: &ContainerId) -> Result<(), BoxErr> {
        self.started_containers
            .lock()
            .unwrap()
            .push(container_id.clone());
        Ok(())
    }

    fn claude_exec_command(
        &self,
        container_id: &ContainerId,
        prompt_file: &str,
        _role: WorkerRole,
    ) -> String {
        format!("mock-exec {container_id} {prompt_file}")
    }

    fn claude_resume_command(
        &self,
        container_id: &ContainerId,
        session_id: &WorkerSessionId,
        _role: WorkerRole,
    ) -> String {
        format!("mock-resume {container_id} {session_id}")
    }

    fn create_container(
        &self,
        _: &str,
        _: &str,
        _: WorkerRole,
        _: &str,
        _: Option<WorkspaceVolume>,
        _: Option<PlanDirMount>,
    ) -> Result<ContainerId, BoxErr> {
        unimplemented!()
    }
    fn stop_container(&self, _: &ContainerId) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn remove_container(&self, _: &ContainerId) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn list_managed_containers(&self) -> Result<Vec<ContainerId>, BoxErr> {
        unimplemented!()
    }
    fn write_settings(&self, _: &ContainerId, _: &Path, _: &str) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn copy_file_to_container(&self, _: &ContainerId, _: &Path, _: &str) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn copy_dir_to_container(&self, _: &ContainerId, _: &Path, _: &str) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn read_container_file(&self, _: &ContainerId, _: &str, _: usize) -> Result<String, BoxErr> {
        unimplemented!()
    }
}
