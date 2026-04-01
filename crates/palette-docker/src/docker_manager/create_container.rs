use super::{DockerManager, run_docker};
use crate::Error;
use palette_domain::worker::{ContainerId, WorkerRole};

/// Workspace volume to mount in the container.
pub struct WorkspaceVolume {
    /// Docker named volume name (e.g., "palette-workspace-C-a3f2b7e1").
    pub name: String,
    /// If true, mount as read-only.
    pub read_only: bool,
}

/// Plan directory bind mount configuration.
pub struct PlanDirMount {
    /// Absolute path on the host (e.g., "/path/to/data/plans").
    pub host_path: String,
    /// If true, mount as read-only.
    pub read_only: bool,
}

impl DockerManager {
    /// Create and start a container for a worker.
    /// Returns the container ID.
    pub fn create_container(
        &self,
        name: &str,
        image: &str,
        role: WorkerRole,
        session_name: &str,
        workspace: Option<WorkspaceVolume>,
        plan_dir: Option<PlanDirMount>,
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
            // Interactive TTY for Claude Code
            "-it".to_string(),
            // Pass Palette API URL as environment variable
            "-e".to_string(),
            format!("PALETTE_URL={}", self.worker_callback_url),
        ];

        if self.callback_network_mode == super::CallbackNetworkMode::Host {
            args.push("--network".to_string());
            args.push("host".to_string());
        }

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
        if role == WorkerRole::Member {
            args.push("-v".to_string());
            args.push("/var/run/docker.sock:/var/run/docker.sock".to_string());
        }

        // Transcript volume: all workers write their transcripts here
        let transcript_volume = format!("palette-transcripts-{session_name}");
        args.push("-v".to_string());
        args.push(format!("{transcript_volume}:/home/agent/.claude/projects"));

        // Workspace volume: shared between worker and reviewer for the same task
        if let Some(ws) = workspace {
            let suffix = if ws.read_only { ":ro" } else { "" };
            args.push("-v".to_string());
            args.push(format!("{}:/home/agent/workspace{suffix}", ws.name));
        }

        // Plan directory: bind mount from host for shared plan documents
        if let Some(pd) = plan_dir {
            let suffix = if pd.read_only { ":ro" } else { "" };
            args.push("-v".to_string());
            args.push(format!("{}:/home/agent/plans{suffix}", pd.host_path));
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
