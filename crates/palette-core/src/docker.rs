use crate::Error;
use crate::models::AgentRole;
use crate::models::ContainerId;
use std::path::Path;
use std::process::Command;

/// Timeout in seconds when stopping a container.
const CONTAINER_STOP_TIMEOUT_SECS: &str = "10";

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
        role: AgentRole,
        session_name: &str,
    ) -> crate::Result<ContainerId> {
        let role_str = role.as_str();
        let labels = [
            "palette.managed=true".to_string(),
            format!("palette.session={session_name}"),
            format!("palette.role={role_str}"),
            format!("palette.agent={name}"),
        ];

        let mut args = vec![
            "create".to_string(),
            "--name".to_string(),
            format!("palette-{name}"),
            // Use host network so 127.0.0.1 reaches the palette server
            // (Claude Code blocks HTTP hooks to private IPs but allows loopback)
            "--network".to_string(),
            "host".to_string(),
            // Interactive TTY for Claude Code
            "-it".to_string(),
            // Pass Palette API URL as environment variable
            "-e".to_string(),
            format!("PALETTE_URL={}", self.palette_url),
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
        if role == AgentRole::Member {
            args.push("-v".to_string());
            args.push("/var/run/docker.sock:/var/run/docker.sock".to_string());
        }

        // Transcript volume: members write, leaders read
        let transcript_volume = format!("palette-transcripts-{session_name}");
        match role {
            AgentRole::Member => {
                args.push("-v".to_string());
                args.push(format!("{transcript_volume}:/home/agent/.claude/projects"));
            }
            AgentRole::Leader => {
                args.push("-v".to_string());
                args.push(format!(
                    "{transcript_volume}:/home/agent/.claude/projects:ro"
                ));
            }
        }

        args.push(image.to_string());
        args.push("sleep".to_string());
        args.push("infinity".to_string());

        let output = run_docker(&args)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Docker(format!(
                "failed to create container palette-{name}: {stderr}"
            )));
        }

        let raw_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let container_id = ContainerId::new(raw_id);
        tracing::info!(container_id = %container_id, name = name, "created container");
        Ok(container_id)
    }

    pub fn start_container(&self, container_id: &ContainerId) -> crate::Result<()> {
        let output = run_docker(["start", container_id.as_ref()])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Docker(format!(
                "failed to start container {container_id}: {stderr}"
            )));
        }
        tracing::info!(container_id = %container_id, "started container");
        Ok(())
    }

    pub fn stop_container(&self, container_id: &ContainerId) -> crate::Result<()> {
        let output = run_docker([
            "stop",
            "-t",
            CONTAINER_STOP_TIMEOUT_SECS,
            container_id.as_ref(),
        ])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!(container_id = %container_id, error = %stderr, "failed to stop container");
        }
        Ok(())
    }

    pub fn remove_container(&self, container_id: &ContainerId) -> crate::Result<()> {
        let output = run_docker(["rm", "-f", container_id.as_ref()])?;
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
            return Err(Error::Docker(format!(
                "failed to write settings.json in container {container_id}: {stderr}"
            )));
        }

        tracing::info!(container_id = %container_id, member_id = member_id, "wrote settings.json");
        Ok(())
    }

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
            return Err(Error::Docker(format!(
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
            return Err(Error::Docker(format!(
                "failed to copy directory to container: {stderr}"
            )));
        }
        Ok(())
    }

    /// Build the command string to launch Claude Code inside a container's tmux pane.
    /// Leaders bypass permissions (they run in a sandbox container).
    /// Members keep default permissions (the leader handles their permission prompts).
    pub fn claude_exec_command(
        container_id: &ContainerId,
        prompt_file: &str,
        role: AgentRole,
    ) -> String {
        let cid = container_id.as_ref();
        let plugin_flag = " --plugin-dir /home/agent/claude-code-plugin";
        match role {
            AgentRole::Leader => {
                format!(
                    "docker exec -it {cid} claude --dangerously-skip-permissions --append-system-prompt-file {prompt_file}{plugin_flag}"
                )
            }
            AgentRole::Member => {
                format!(
                    "docker exec -it {cid} claude --append-system-prompt-file {prompt_file}{plugin_flag}"
                )
            }
        }
    }
}

fn run_docker<I, S>(args: I) -> crate::Result<std::process::Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    Ok(Command::new("docker").args(args).output()?)
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

    #[test]
    fn claude_exec_command_leader_bypasses_permissions() {
        let cid = ContainerId::new("abc123");
        let cmd = DockerManager::claude_exec_command(
            &cid,
            "/home/agent/prompts/leader.md",
            AgentRole::Leader,
        );
        assert!(cmd.contains("docker exec -it abc123 claude"));
        assert!(cmd.contains("--dangerously-skip-permissions"));
        assert!(cmd.contains("--append-system-prompt-file /home/agent/prompts/leader.md"));
        assert!(cmd.contains("--plugin-dir /home/agent/claude-code-plugin"));
    }

    #[test]
    fn claude_exec_command_member_keeps_permissions() {
        let cid = ContainerId::new("abc123");
        let cmd = DockerManager::claude_exec_command(
            &cid,
            "/home/agent/prompts/member.md",
            AgentRole::Member,
        );
        assert!(cmd.contains("docker exec -it abc123 claude"));
        assert!(!cmd.contains("--dangerously-skip-permissions"));
        assert!(cmd.contains("--append-system-prompt-file /home/agent/prompts/member.md"));
        assert!(cmd.contains("--plugin-dir /home/agent/claude-code-plugin"));
    }
}
