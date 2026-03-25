use super::{DockerManager, run_docker};
use crate::Error;
use palette_domain::worker::ContainerId;

impl DockerManager {
    pub fn start_container(&self, container_id: &ContainerId) -> crate::Result<()> {
        let output = run_docker(["start", container_id.as_ref()])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to start container {container_id}: {stderr}"
            )));
        }

        // Wait until the container is actually running before returning.
        // `docker start` can return before the entrypoint process is ready,
        // causing subsequent `docker exec` calls to fail.
        let cid = container_id.as_ref();
        for _ in 0..20 {
            let inspect = run_docker(["inspect", "-f", "{{.State.Running}}", cid])?;
            let running = String::from_utf8_lossy(&inspect.stdout).trim().to_string();
            if running == "true" {
                tracing::info!(container_id = %container_id, "started container");
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        Err(Error::Command(format!(
            "container {container_id} did not reach running state"
        )))
    }
}
