use super::{DockerManager, run_docker};
use palette_domain::worker::ContainerId;

impl DockerManager {
    /// Check whether a Claude Code process is running inside the container.
    pub fn is_claude_running(container_id: &ContainerId) -> bool {
        let cid = container_id.as_ref();
        let output = run_docker(["exec", cid, "pgrep", "-f", "claude"]);
        match output {
            Ok(out) => out.status.success(),
            Err(e) => {
                tracing::warn!(container_id = cid, error = %e, "failed to check if claude is running");
                false
            }
        }
    }
}
