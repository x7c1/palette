use super::{DockerManager, run_docker};
use crate::Error;
use palette_domain::agent::{AgentRole, ContainerId};

/// Workspace volume to mount in the container.
pub struct WorkspaceVolume {
    /// Docker named volume name (e.g., "palette-workspace-W-001").
    pub name: String,
    /// If true, mount as read-only.
    pub read_only: bool,
}

impl DockerManager {
    /// Create and start a container for an agent.
    /// Returns the container ID.
    pub fn create_container(
        &self,
        name: &str,
        image: &str,
        role: AgentRole,
        session_name: &str,
        workspace: Option<WorkspaceVolume>,
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

        // Transcript volume: all agents write their transcripts here
        let transcript_volume = format!("palette-transcripts-{session_name}");
        args.push("-v".to_string());
        args.push(format!("{transcript_volume}:/home/agent/.claude/projects"));

        // Workspace volume: shared between worker and reviewer for the same task
        if let Some(ws) = workspace {
            let suffix = if ws.read_only { ":ro" } else { "" };
            args.push("-v".to_string());
            args.push(format!("{}:/home/agent/workspace{suffix}", ws.name));
        }

        args.push(image.to_string());

        // Fix workspace ownership then idle; named volumes may mount as root
        // even though Dockerfile.base pre-creates the directory as agent user.
        // Use semicolon (not &&) because chown fails on read-only mounts,
        // which is expected for reviewer containers.
        args.push("sh".to_string());
        args.push("-c".to_string());
        args.push(
            "sudo chown agent:agent /home/agent/workspace 2>/dev/null; exec sleep infinity"
                .to_string(),
        );

        let output = run_docker(&args)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to create container palette-{name}: {stderr}"
            )));
        }

        let raw_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let container_id = ContainerId::new(raw_id);
        tracing::info!(container_id = %container_id, name = name, "created container");
        Ok(container_id)
    }
}
