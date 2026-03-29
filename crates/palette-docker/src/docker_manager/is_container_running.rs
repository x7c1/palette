use super::run_docker;

/// Check if a Docker container is running.
pub fn is_container_running(container_id: &str) -> bool {
    let output = run_docker(["inspect", "-f", "{{.State.Running}}", container_id]);
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.trim() == "true"
        }
        Err(e) => {
            tracing::warn!(container_id, error = %e, "failed to inspect container");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonexistent_container_returns_false() {
        assert!(!is_container_running("nonexistent-container-id-12345"));
    }
}
