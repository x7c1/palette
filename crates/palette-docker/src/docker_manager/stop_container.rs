use super::{DockerManager, run_docker};
use palette_domain::agent::ContainerId;

/// Timeout in seconds when stopping a container.
const CONTAINER_STOP_TIMEOUT_SECS: &str = "10";

impl DockerManager {
    pub fn stop_container(&self, container_id: &ContainerId) -> crate::Result<()> {
        let output = run_docker([
            "stop",
            "-t",
            CONTAINER_STOP_TIMEOUT_SECS,
            container_id.as_ref(),
        ])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!(container_id = %container_id, error = %stderr, "failed to stop container");
        }
        Ok(())
    }
}
