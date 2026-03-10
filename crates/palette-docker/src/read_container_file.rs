use palette_domain::agent::ContainerId;

/// Read the last N lines of a file inside a running container.
pub fn read_container_file(
    container_id: &ContainerId,
    path: &str,
    tail_lines: usize,
) -> crate::Result<String> {
    let output = std::process::Command::new("docker")
        .args([
            "exec",
            container_id.as_ref(),
            "tail",
            &format!("-{tail_lines}"),
            path,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::Error::Command(format!(
            "failed to read {path} from container {container_id}: {stderr}"
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
