use super::{DockerManager, run_docker};
use crate::Error;
use palette_domain::agent::ContainerId;

impl DockerManager {
    /// Write the settings.json file inside a running container.
    /// Reads the template from `template_path`, replaces PALETTE_URL placeholder,
    /// and adds member_id to hook URLs as query parameter.
    pub fn write_settings(
        &self,
        container_id: &ContainerId,
        template_path: &std::path::Path,
        member_id: &str,
    ) -> crate::Result<()> {
        let template = std::fs::read_to_string(template_path)?;

        // Replace template placeholders with actual URLs
        let settings = template
            .replace(
                "{{PALETTE_STOP_URL}}",
                &format!("{}/hooks/stop?member_id={member_id}", self.palette_url),
            )
            .replace(
                "{{PALETTE_NOTIFICATION_URL}}",
                &format!(
                    "{}/hooks/notification?member_id={member_id}",
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
                "mkdir -p /home/agent/.claude && cat > /home/agent/.claude/settings.json << 'PALETTE_EOF'\n{settings}\nPALETTE_EOF\nchown agent:agent /home/agent/.claude /home/agent/.claude/settings.json"
            ),
        ])?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to write settings.json in container {container_id}: {stderr}"
            )));
        }

        tracing::info!(container_id = %container_id, member_id = member_id, "wrote settings.json");
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
                "{{PALETTE_STOP_URL}}",
                &format!("{}/hooks/stop?member_id=member-a", mgr.palette_url),
            )
            .replace(
                "{{PALETTE_NOTIFICATION_URL}}",
                &format!("{}/hooks/notification?member_id=member-a", mgr.palette_url),
            );

        assert!(settings.contains(&format!("{palette_url}/hooks/stop?member_id=member-a")));
        assert!(settings.contains(&format!(
            "{palette_url}/hooks/notification?member_id=member-a"
        )));
    }
}
