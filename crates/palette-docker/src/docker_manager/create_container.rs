use super::{DockerManager, run_docker};
use crate::Error;
use palette_domain::worker::{ContainerId, WorkerRole};
use std::path::{Path, PathBuf};

/// Workspace bind mount configuration for the container.
pub struct WorkspaceVolume {
    /// Absolute path on the host to the workspace directory.
    pub host_path: String,
    /// Absolute path on the host to the bare repository cache.
    pub repo_cache_path: String,
    /// If true, mount workspace as read-only (for reviewers).
    pub read_only: bool,
}

/// Plan directory bind mount configuration.
pub struct PlanDirMount {
    /// Absolute path on the host (e.g., "/path/to/data/plans").
    pub host_path: String,
    /// If true, mount as read-only.
    pub read_only: bool,
}

/// Artifacts directory bind mount configuration.
pub struct ArtifactsMount {
    /// Absolute path on the host to the artifacts directory.
    pub host_path: String,
    /// If true, mount as read-only.
    pub read_only: bool,
}

impl DockerManager {
    /// Create and start a container for a worker.
    /// Returns the container ID.
    #[allow(clippy::too_many_arguments)]
    pub fn create_container(
        &self,
        name: &str,
        image: &str,
        role: WorkerRole,
        session_name: &str,
        workspace: Option<WorkspaceVolume>,
        plan_dir: Option<PlanDirMount>,
        artifacts_dir: Option<ArtifactsMount>,
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
        let home_path = Path::new(&home);
        let fail_fast_credentials = env_flag("PALETTE_FAIL_FAST_CREDENTIALS");

        let mut auth_marker_found = false;
        for mount in resolve_claude_auth_mounts(home_path) {
            if mount.is_auth_marker {
                auth_marker_found = true;
            }
            args.push("-v".to_string());
            args.push(format!(
                "{}:{}:ro",
                mount.host_path.display(),
                mount.container_path
            ));
        }

        // Git config
        let git_config = format!("{home}/.config/git");
        if std::path::Path::new(&git_config).exists() {
            args.push("-v".to_string());
            args.push(format!("{git_config}:/home/agent/.config/git:ro"));
        } else if fail_fast_credentials {
            return Err(Error::Command(
                "missing required git config: ~/.config/git. migrate from ~/.gitconfig to XDG path"
                    .to_string(),
            ));
        }

        // GitHub CLI config
        let gh_config = format!("{home}/.config/gh");
        if std::path::Path::new(&gh_config).exists() {
            args.push("-v".to_string());
            args.push(format!("{gh_config}:/home/agent/.config/gh:ro"));
        }

        // SSH config/keys are optional, but can be mounted for SSH remotes.
        let ssh_dir = format!("{home}/.ssh");
        if std::path::Path::new(&ssh_dir).exists() {
            args.push("-v".to_string());
            args.push(format!("{ssh_dir}:/home/agent/.ssh:ro"));
        }

        if fail_fast_credentials && !auth_marker_found {
            return Err(Error::Command(
                "missing Claude auth bundle/credentials. run bootstrap login and provide auth files"
                    .to_string(),
            ));
        }

        // Docker socket for members
        if role == WorkerRole::Member {
            args.push("-v".to_string());
            args.push("/var/run/docker.sock:/var/run/docker.sock".to_string());
        }

        // Transcript directory: bind mount to host for persistence after container removal.
        // Each worker gets its own subdirectory under data/transcripts/{worker_name}/.
        let transcript_host_dir = format!("data/transcripts/{name}");
        std::fs::create_dir_all(&transcript_host_dir).ok();
        let transcript_abs = std::fs::canonicalize(&transcript_host_dir)
            .unwrap_or_else(|_| std::path::PathBuf::from(&transcript_host_dir));
        args.push("-v".to_string());
        args.push(format!(
            "{}:/home/agent/.claude/projects",
            transcript_abs.display()
        ));

        // Workspace: bind mount from host directory
        if let Some(ws) = &workspace {
            let suffix = if ws.read_only { ":ro" } else { "" };
            args.push("-v".to_string());
            args.push(format!("{}:/home/agent/workspace{suffix}", ws.host_path));
            // Bare repository cache: always read-only inside the container
            args.push("-v".to_string());
            args.push(format!("{}:/home/agent/repo-cache:ro", ws.repo_cache_path));
        }

        // Plan directory: bind mount from host for shared plan documents
        if let Some(pd) = plan_dir {
            let suffix = if pd.read_only { ":ro" } else { "" };
            args.push("-v".to_string());
            args.push(format!("{}:/home/agent/plans{suffix}", pd.host_path));
        }

        // Artifacts directory: review results and check outputs
        if let Some(ad) = artifacts_dir {
            let suffix = if ad.read_only { ":ro" } else { "" };
            args.push("-v".to_string());
            args.push(format!("{}:/home/agent/artifacts{suffix}", ad.host_path));
        }

        args.push(image.to_string());

        // Fix workspace ownership then idle; bind mounts inherit host
        // ownership which may differ from the container's agent user.
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

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

struct AuthMount {
    host_path: PathBuf,
    container_path: &'static str,
    is_auth_marker: bool,
}

fn resolve_claude_auth_mounts(home: &Path) -> Vec<AuthMount> {
    let auth_bundle_root = std::env::var("PALETTE_CLAUDE_AUTH_BUNDLE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".config/palette/claude-auth-bundle"));
    resolve_claude_auth_mounts_with_root(home, &auth_bundle_root)
}

fn resolve_claude_auth_mounts_with_root(home: &Path, auth_bundle_root: &Path) -> Vec<AuthMount> {
    let mut mounts = Vec::new();

    let mut add_mount = |host_rel: &str, container_path: &'static str, is_auth_marker: bool| {
        let path = auth_bundle_root.join(host_rel);
        if path.exists() {
            mounts.push(AuthMount {
                host_path: path,
                container_path,
                is_auth_marker,
            });
        }
    };

    add_mount(
        ".claude/.credentials.json",
        "/home/agent/.claude/.credentials.json",
        true,
    );
    add_mount(".claude/settings.json", "/home/agent/.claude/settings.json", true);
    add_mount(".claude/CLAUDE.md", "/home/agent/.claude/CLAUDE.md", false);
    add_mount(".claude.json", "/home/agent/.claude.json", false);

    if !mounts.iter().any(|m| m.is_auth_marker) {
        let legacy = home.join(".claude/.credentials.json");
        if legacy.exists() {
            mounts.push(AuthMount {
                host_path: legacy,
                container_path: "/home/agent/.claude/.credentials.json",
                is_auth_marker: true,
            });
        }
    }

    mounts
}

#[cfg(test)]
mod tests {
    use super::resolve_claude_auth_mounts_with_root;
    use std::fs;

    #[test]
    fn prefers_bundle_paths_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let bundle = home.join(".config/palette/claude-auth-bundle/.claude");
        fs::create_dir_all(&bundle).unwrap();
        fs::write(bundle.join(".credentials.json"), "{}").unwrap();
        fs::write(bundle.join("settings.json"), "{}").unwrap();

        let mounts = resolve_claude_auth_mounts_with_root(
            home,
            &home.join(".config/palette/claude-auth-bundle"),
        );
        assert_eq!(mounts.len(), 2);
        assert!(mounts
            .iter()
            .any(|m| m.container_path == "/home/agent/.claude/.credentials.json"));
        assert!(mounts
            .iter()
            .any(|m| m.container_path == "/home/agent/.claude/settings.json"));
    }

    #[test]
    fn falls_back_to_legacy_credentials_json() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let legacy_dir = home.join(".claude");
        fs::create_dir_all(&legacy_dir).unwrap();
        fs::write(legacy_dir.join(".credentials.json"), "{}").unwrap();

        let mounts = resolve_claude_auth_mounts_with_root(
            home,
            &home.join(".config/palette/claude-auth-bundle"),
        );
        assert_eq!(mounts.len(), 1);
        assert_eq!(
            mounts[0].container_path,
            "/home/agent/.claude/.credentials.json"
        );
    }
}
