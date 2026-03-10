use super::DockerManager;
use crate::Error;
use palette_domain::agent::ContainerId;
use std::path::Path;
use std::process::Command;

impl DockerManager {
    /// Copy a directory tree into a container.
    pub fn copy_dir_to_container(
        container_id: &ContainerId,
        local_dir: &Path,
        container_path: &str,
    ) -> crate::Result<()> {
        let cid = container_id.as_ref();
        let output = Command::new("docker")
            .args([
                "cp",
                &format!("{}/.", local_dir.display()),
                &format!("{cid}:{container_path}"),
            ])
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to copy directory to container: {stderr}"
            )));
        }
        Ok(())
    }
}
