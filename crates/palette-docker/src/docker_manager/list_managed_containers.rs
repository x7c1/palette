use super::{DockerManager, run_docker};
use palette_domain::worker::ContainerId;

impl DockerManager {
    /// List all containers with the `palette.managed=true` label.
    /// Includes both running and stopped containers.
    pub fn list_managed_containers(&self) -> crate::Result<Vec<ContainerId>> {
        let output = run_docker([
            "ps",
            "-a",
            "--filter",
            "label=palette.managed=true",
            "--format",
            "{{.ID}}",
        ])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::Error::Command(format!(
                "failed to list managed containers: {stderr}"
            )));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let ids = stdout
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| ContainerId::new(line.to_string()))
            .collect();
        Ok(ids)
    }
}
