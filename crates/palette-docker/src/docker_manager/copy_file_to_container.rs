use super::{DockerManager, run_docker};
use crate::Error;
use palette_domain::agent::ContainerId;

impl DockerManager {
    /// Copy a local file into a running container.
    pub fn copy_file_to_container(
        container_id: &ContainerId,
        local_path: &std::path::Path,
        container_path: &str,
    ) -> crate::Result<()> {
        let cid = container_id.as_ref();
        let output = run_docker([
            "cp",
            &local_path.display().to_string(),
            &format!("{cid}:{container_path}"),
        ])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to copy {} to {container_id}:{container_path}: {stderr}",
                local_path.display()
            )));
        }
        tracing::info!(
            container_id = %container_id,
            path = container_path,
            "copied file to container"
        );
        Ok(())
    }
}
