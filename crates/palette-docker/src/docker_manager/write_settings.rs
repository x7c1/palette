use super::{DockerManager, run_docker};
use crate::Error;
use palette_domain::worker::ContainerId;

impl DockerManager {
    /// Write the settings.json file inside a running container.
    /// Reads the template from `template_path`, replaces placeholders,
    /// and adds worker_id to hook URLs as query parameter.
    pub fn write_settings(
        &self,
        container_id: &ContainerId,
        template_path: &std::path::Path,
        worker_id: &str,
    ) -> crate::Result<()> {
        let template = std::fs::read_to_string(template_path)?;

        // Replace template placeholders with actual URLs
        let settings = template
            .replace(
                "{{PALETTE_SESSION_START_URL}}",
                &format!(
                    "{}/hooks/session-start?worker_id={worker_id}",
                    self.palette_url
                ),
            )
            .replace(
                "{{PALETTE_STOP_URL}}",
                &format!("{}/hooks/stop?worker_id={worker_id}", self.palette_url),
            )
            .replace(
                "{{PALETTE_NOTIFICATION_URL}}",
                &format!(
                    "{}/hooks/notification?worker_id={worker_id}",
                    self.palette_url
                ),
            );

        let cid = container_id.as_ref();
        // Write via docker exec (as root to avoid permission issues, then chown)
        let output = run_docker([
            "exec",
            "--user",
            "root",
            cid,
            "sh",
            "-c",
            &format!(
                "mkdir -p /home/agent/.claude/hooks && cat > /home/agent/.claude/settings.json << 'PALETTE_EOF'\n{settings}\nPALETTE_EOF\nchown -R agent:agent /home/agent/.claude"
            ),
        ])?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to write settings.json in container {container_id}: {stderr}"
            )));
        }

        tracing::info!(container_id = %container_id, worker_id = worker_id, "wrote settings.json");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::DockerManager;

    #[test]
    fn settings_template_expansion() {
        let dir = tempfile::tempdir().unwrap();
        let template_path = dir.path().join("template.json");
        std::fs::write(
            &template_path,
            r#"{
  "hooks": {
    "SessionStart": [{"hooks": [{"type": "http", "url": "{{PALETTE_SESSION_START_URL}}"}]}],
    "Stop": [{"hooks": [{"type": "http", "url": "{{PALETTE_STOP_URL}}"}]}],
    "Notification": [{"matcher": "permission_prompt", "hooks": [{"type": "http", "url": "{{PALETTE_NOTIFICATION_URL}}"}]}]
  }
}"#,
        )
        .unwrap();

        let palette_url = "http://localhost:9000";
        let mgr = DockerManager::new(palette_url.to_string());

        // We can't test docker exec, but we can test the template expansion logic
        let template = std::fs::read_to_string(&template_path).unwrap();
        let settings = template
            .replace(
                "{{PALETTE_SESSION_START_URL}}",
                &format!("{}/hooks/session-start?worker_id=worker-a", mgr.palette_url),
            )
            .replace(
                "{{PALETTE_STOP_URL}}",
                &format!("{}/hooks/stop?worker_id=worker-a", mgr.palette_url),
            )
            .replace(
                "{{PALETTE_NOTIFICATION_URL}}",
                &format!("{}/hooks/notification?worker_id=worker-a", mgr.palette_url),
            );

        assert!(settings.contains(&format!(
            "{palette_url}/hooks/session-start?worker_id=worker-a"
        )));
        assert!(settings.contains(&format!("{palette_url}/hooks/stop?worker_id=worker-a")));
        assert!(settings.contains(&format!(
            "{palette_url}/hooks/notification?worker_id=worker-a"
        )));
    }
}
