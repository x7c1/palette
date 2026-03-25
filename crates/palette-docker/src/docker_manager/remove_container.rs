use super::{DockerManager, run_docker};
use palette_domain::worker::ContainerId;

impl DockerManager {
    pub fn remove_container(&self, container_id: &ContainerId) -> crate::Result<()> {
        let output = run_docker(["rm", "-f", container_id.as_ref()])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!(container_id = %container_id, error = %stderr, "failed to remove container");
        }
        Ok(())
    }
}
