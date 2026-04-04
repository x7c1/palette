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
        let session_start_url = format!(
            "{}/hooks/session-start?worker_id={worker_id}",
            self.worker_callback_url
        );
        let stop_url = format!("{}/hooks/stop?worker_id={worker_id}", self.worker_callback_url);
        let notification_url = format!(
            "{}/hooks/notification?worker_id={worker_id}",
            self.worker_callback_url
        );

        // Replace template placeholders with actual URLs
        let settings = template
            .replace("{{PALETTE_SESSION_START_URL}}", &session_start_url)
            .replace("{{PALETTE_STOP_URL}}", &stop_url)
            .replace("{{PALETTE_NOTIFICATION_URL}}", &notification_url);

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
                "mkdir -p /home/agent/.claude/hooks && cat > /home/agent/.claude/settings.json << 'PALETTE_EOF'\n{settings}\nPALETTE_EOF\nchown agent:agent /home/agent/.claude /home/agent/.claude/settings.json /home/agent/.claude/hooks"
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
    use super::super::{CallbackNetworkMode, DockerManager};

    #[test]
    fn settings_template_expansion() {
        let dir = tempfile::tempdir().unwrap();
        let template_path = dir.path().join("template.json");
        std::fs::write(
            &template_path,
            r#"{
  "hooks": {
    "SessionStart": [{"hooks": [{"type": "http", "url": "{{PALETTE_SESSION_START_URL}}"}]}],
    "Stop": [{"hooks": [{"type": "command", "command": "curl -sf -X POST -H 'Content-Type: application/json' -d @- '{{PALETTE_STOP_URL}}' || true"}]}],
    "Notification": [{"matcher": "permission_prompt", "hooks": [{"type": "command", "command": "curl -sf -X POST -H 'Content-Type: application/json' -d @- '{{PALETTE_NOTIFICATION_URL}}' || true"}]}]
  }
}"#,
        )
        .unwrap();

        let callback_url = "http://localhost:9000";
        let mgr = DockerManager::new(callback_url.to_string(), CallbackNetworkMode::Host);

        // We can't test docker exec, but we can test the template expansion logic
        let template = std::fs::read_to_string(&template_path).unwrap();
        let settings = template
            .replace(
                "{{PALETTE_SESSION_START_URL}}",
                &format!(
                    "{}/hooks/session-start?worker_id=worker-a",
                    mgr.worker_callback_url
                ),
            )
            .replace(
                "{{PALETTE_STOP_URL}}",
                &format!("{}/hooks/stop?worker_id=worker-a", mgr.worker_callback_url),
            )
            .replace(
                "{{PALETTE_NOTIFICATION_URL}}",
                &format!(
                    "{}/hooks/notification?worker_id=worker-a",
                    mgr.worker_callback_url
                ),
            );

        assert!(settings.contains(&format!(
            "{callback_url}/hooks/session-start?worker_id=worker-a"
        )));
        assert!(settings.contains("\"type\": \"command\""));
        assert!(settings.contains(&format!(
            "'{callback_url}/hooks/stop?worker_id=worker-a' || true"
        )));
        assert!(settings.contains(&format!(
            "'{callback_url}/hooks/notification?worker_id=worker-a' || true"
        )));
    }
}
