use anyhow::{Context as _, bail};
use std::process::Command;

pub struct DockerManager {
    palette_url: String,
}

impl DockerManager {
    pub fn new(palette_url: String) -> Self {
        Self { palette_url }
    }

    /// Create and start a container for an agent.
    /// Returns the container ID.
    pub fn create_container(
        &self,
        name: &str,
        image: &str,
        role: &str,
        session_name: &str,
    ) -> anyhow::Result<String> {
        let labels = [
            "palette.managed=true".to_string(),
            format!("palette.session={session_name}"),
            format!("palette.role={role}"),
            format!("palette.agent={name}"),
        ];

        let mut args = vec![
            "create".to_string(),
            "--name".to_string(),
            format!("palette-{name}"),
            "--hostname".to_string(),
            format!("palette-{name}"),
            // Linux: map host.docker.internal to host gateway
            "--add-host=host.docker.internal:host-gateway".to_string(),
            // Interactive TTY for Claude Code
            "-it".to_string(),
        ];

        for label in &labels {
            args.push("--label".to_string());
            args.push(label.clone());
        }

        // Mount host authentication files (read-only)
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());

        // Claude credentials
        let creds_path = format!("{home}/.claude/.credentials.json");
        if std::path::Path::new(&creds_path).exists() {
            args.push("-v".to_string());
            args.push(format!(
                "{creds_path}:/home/agent/.claude/.credentials.json:ro"
            ));
        }

        // Git config
        let git_config = format!("{home}/.config/git");
        if std::path::Path::new(&git_config).exists() {
            args.push("-v".to_string());
            args.push(format!("{git_config}:/home/agent/.config/git:ro"));
        }

        // GitHub CLI config
        let gh_config = format!("{home}/.config/gh");
        if std::path::Path::new(&gh_config).exists() {
            args.push("-v".to_string());
            args.push(format!("{gh_config}:/home/agent/.config/gh:ro"));
        }

        // Docker socket for members
        if role == "member" {
            args.push("-v".to_string());
            args.push("/var/run/docker.sock:/var/run/docker.sock".to_string());
        }

        args.push(image.to_string());
        args.push("sleep".to_string());
        args.push("infinity".to_string());

        let output = run_docker(&args)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("failed to create container palette-{name}: {stderr}");
        }

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        tracing::info!(container_id = %container_id, name = name, "created container");
        Ok(container_id)
    }

    pub fn start_container(&self, container_id: &str) -> anyhow::Result<()> {
        let output = run_docker(&["start", container_id])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("failed to start container {container_id}: {stderr}");
        }
        tracing::info!(container_id = %container_id, "started container");
        Ok(())
    }

    pub fn stop_container(&self, container_id: &str) -> anyhow::Result<()> {
        let output = run_docker(&["stop", "-t", "10", container_id])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!(container_id = %container_id, error = %stderr, "failed to stop container");
        }
        Ok(())
    }

    pub fn remove_container(&self, container_id: &str) -> anyhow::Result<()> {
        let output = run_docker(&["rm", "-f", container_id])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!(container_id = %container_id, error = %stderr, "failed to remove container");
        }
        Ok(())
    }

    /// Write the settings.json file inside a running container.
    /// Reads the template from `template_path`, replaces PALETTE_URL placeholder,
    /// and adds member_id to hook URLs as query parameter.
    pub fn write_settings(
        &self,
        container_id: &str,
        template_path: &std::path::Path,
        member_id: &str,
    ) -> anyhow::Result<()> {
        let template = std::fs::read_to_string(template_path).with_context(|| {
            format!(
                "failed to read settings template: {}",
                template_path.display()
            )
        })?;

        // Replace the base URL and add member_id query param
        let settings = template
            .replace(
                "http://host.docker.internal:7100/hooks/stop",
                &format!("{}/hooks/stop?member_id={member_id}", self.palette_url),
            )
            .replace(
                "http://host.docker.internal:7100/hooks/notification",
                &format!(
                    "{}/hooks/notification?member_id={member_id}",
                    self.palette_url
                ),
            );

        // Write via docker exec
        let output = run_docker(&[
            "exec",
            container_id,
            "sh",
            "-c",
            &format!(
                "mkdir -p /home/agent/.claude && cat > /home/agent/.claude/settings.json << 'PALETTE_EOF'\n{settings}\nPALETTE_EOF"
            ),
        ])?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("failed to write settings.json in container {container_id}: {stderr}");
        }

        tracing::info!(container_id = %container_id, member_id = member_id, "wrote settings.json");
        Ok(())
    }

    /// Copy a local file into a running container.
    pub fn copy_file_to_container(
        container_id: &str,
        local_path: &std::path::Path,
        container_path: &str,
    ) -> anyhow::Result<()> {
        let output = run_docker(&[
            "cp",
            &local_path.display().to_string(),
            &format!("{container_id}:{container_path}"),
        ])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "failed to copy {} to {container_id}:{container_path}: {stderr}",
                local_path.display()
            );
        }
        tracing::info!(
            container_id = %container_id,
            path = container_path,
            "copied file to container"
        );
        Ok(())
    }

    /// Build the command string to launch Claude Code inside a container's tmux pane.
    pub fn claude_exec_command(container_id: &str, prompt_file: &str) -> String {
        format!(
            "docker exec -it {container_id} claude --dangerously-skip-permissions --append-system-prompt-file {prompt_file}"
        )
    }
}

fn run_docker<I, S>(args: I) -> anyhow::Result<std::process::Output>
where
    I: IntoIterator<Item = S> + std::fmt::Debug + Clone,
    S: AsRef<std::ffi::OsStr>,
{
    Command::new("docker")
        .args(args.clone())
        .output()
        .with_context(|| format!("failed to run docker {args:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_template_expansion() {
        let dir = tempfile::tempdir().unwrap();
        let template_path = dir.path().join("template.json");
        std::fs::write(
            &template_path,
            r#"{
  "hooks": {
    "Stop": [{"hooks": [{"type": "http", "url": "http://host.docker.internal:7100/hooks/stop"}]}],
    "Notification": [{"matcher": "permission_prompt", "hooks": [{"type": "http", "url": "http://host.docker.internal:7100/hooks/notification"}]}]
  }
}"#,
        )
        .unwrap();

        let mgr = DockerManager::new("http://host.docker.internal:9000".to_string());

        // We can't test docker exec, but we can test the template expansion logic
        let template = std::fs::read_to_string(&template_path).unwrap();
        let settings = template
            .replace(
                "http://host.docker.internal:7100/hooks/stop",
                &format!("{}/hooks/stop?member_id=member-a", mgr.palette_url),
            )
            .replace(
                "http://host.docker.internal:7100/hooks/notification",
                &format!("{}/hooks/notification?member_id=member-a", mgr.palette_url),
            );

        assert!(
            settings.contains("http://host.docker.internal:9000/hooks/stop?member_id=member-a")
        );
        assert!(
            settings
                .contains("http://host.docker.internal:9000/hooks/notification?member_id=member-a")
        );
    }

    #[test]
    fn claude_exec_command_format() {
        let cmd = DockerManager::claude_exec_command("abc123", "/home/agent/prompts/leader.md");
        assert!(cmd.contains("docker exec -it abc123 claude"));
        assert!(cmd.contains("--append-system-prompt-file /home/agent/prompts/leader.md"));
    }
}
