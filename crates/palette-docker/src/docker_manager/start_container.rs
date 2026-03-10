use super::{DockerManager, run_docker};
use crate::Error;
use palette_domain::agent::ContainerId;

impl DockerManager {
    pub fn start_container(&self, container_id: &ContainerId) -> crate::Result<()> {
        let output = run_docker(["start", container_id.as_ref()])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to start container {container_id}: {stderr}"
            )));
        }
        tracing::info!(container_id = %container_id, "started container");
        Ok(())
    }
}
