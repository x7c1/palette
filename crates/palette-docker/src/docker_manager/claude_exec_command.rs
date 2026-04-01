use super::DockerManager;
use palette_domain::worker::{ContainerId, WorkerRole, WorkerSessionId};

impl DockerManager {
    /// Build the command string to launch Claude Code inside a container's tmux pane.
    ///
    /// `workdir` sets the initial working directory via `docker exec --workdir`.
    /// Pass the container-side workspace path (e.g. `/home/agent/workspace`) for
    /// crafters and reviewers so they start inside the repo without needing `cd`.
    pub fn claude_exec_command(
        container_id: &ContainerId,
        prompt_file: &str,
        role: WorkerRole,
        workdir: Option<&str>,
    ) -> String {
        let cid = container_id.as_ref();
        let wd = workdir
            .map(|d| format!(" --workdir {d}"))
            .unwrap_or_default();
        let plugin_flag = " --plugin-dir /home/agent/claude-code-plugin";
        if role.skip_permissions() {
            format!(
                "docker exec -it{wd} {cid} claude --dangerously-skip-permissions --append-system-prompt-file {prompt_file}{plugin_flag}"
            )
        } else {
            format!(
                "docker exec -it{wd} {cid} claude --append-system-prompt-file {prompt_file}{plugin_flag}"
            )
        }
    }

    /// Build the command string to resume a Claude Code session inside a container.
    /// Used for crash recovery when a session_id is available.
    pub fn claude_resume_command(
        container_id: &ContainerId,
        session_id: &WorkerSessionId,
        role: WorkerRole,
        workdir: Option<&str>,
    ) -> String {
        let cid = container_id.as_ref();
        let sid = session_id.as_ref();
        let wd = workdir
            .map(|d| format!(" --workdir {d}"))
            .unwrap_or_default();
        let plugin_flag = " --plugin-dir /home/agent/claude-code-plugin";
        if role.skip_permissions() {
            format!(
                "docker exec -it{wd} {cid} claude --resume {sid} --dangerously-skip-permissions{plugin_flag}"
            )
        } else {
            format!("docker exec -it{wd} {cid} claude --resume {sid}{plugin_flag}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::DockerManager;
    use palette_domain::worker::{ContainerId, WorkerRole, WorkerSessionId};

    #[test]
    fn permission_supervisor_bypasses_permissions() {
        let cid = ContainerId::new("abc123");
        let cmd = DockerManager::claude_exec_command(
            &cid,
            "/home/agent/prompts/permission-supervisor.md",
            WorkerRole::PermissionSupervisor,
            None,
        );
        assert!(cmd.contains("docker exec -it abc123 claude"));
        assert!(cmd.contains("--dangerously-skip-permissions"));
        assert!(
            cmd.contains(
                "--append-system-prompt-file /home/agent/prompts/permission-supervisor.md"
            )
        );
        assert!(cmd.contains("--plugin-dir /home/agent/claude-code-plugin"));
    }

    #[test]
    fn member_keeps_permissions() {
        let cid = ContainerId::new("abc123");
        let cmd = DockerManager::claude_exec_command(
            &cid,
            "/home/agent/prompts/member.md",
            WorkerRole::Member,
            None,
        );
        assert!(cmd.contains("docker exec -it abc123 claude"));
        assert!(!cmd.contains("--dangerously-skip-permissions"));
        assert!(cmd.contains("--append-system-prompt-file /home/agent/prompts/member.md"));
        assert!(cmd.contains("--plugin-dir /home/agent/claude-code-plugin"));
    }

    #[test]
    fn member_with_workdir() {
        let cid = ContainerId::new("abc123");
        let cmd = DockerManager::claude_exec_command(
            &cid,
            "/home/agent/prompts/member.md",
            WorkerRole::Member,
            Some("/home/agent/workspace"),
        );
        assert!(cmd.contains("--workdir /home/agent/workspace"));
        assert!(cmd.contains("abc123 claude"));
    }

    #[test]
    fn review_integrator_bypasses_permissions() {
        let cid = ContainerId::new("abc123");
        let cmd = DockerManager::claude_exec_command(
            &cid,
            "/home/agent/prompts/review-integrator.md",
            WorkerRole::ReviewIntegrator,
            None,
        );
        assert!(cmd.contains("docker exec -it abc123 claude"));
        assert!(cmd.contains("--dangerously-skip-permissions"));
        assert!(
            cmd.contains("--append-system-prompt-file /home/agent/prompts/review-integrator.md")
        );
    }

    #[test]
    fn resume_member_session() {
        let cid = ContainerId::new("abc123");
        let sid = WorkerSessionId::new("session-xyz");
        let cmd = DockerManager::claude_resume_command(&cid, &sid, WorkerRole::Member, None);
        assert!(cmd.contains("docker exec -it abc123 claude"));
        assert!(cmd.contains("--resume session-xyz"));
        assert!(!cmd.contains("--dangerously-skip-permissions"));
        assert!(cmd.contains("--plugin-dir /home/agent/claude-code-plugin"));
    }

    #[test]
    fn resume_permission_supervisor_session() {
        let cid = ContainerId::new("abc123");
        let sid = WorkerSessionId::new("session-xyz");
        let cmd = DockerManager::claude_resume_command(
            &cid,
            &sid,
            WorkerRole::PermissionSupervisor,
            None,
        );
        assert!(cmd.contains("--resume session-xyz"));
        assert!(cmd.contains("--dangerously-skip-permissions"));
    }

    #[test]
    fn resume_with_workdir() {
        let cid = ContainerId::new("abc123");
        let sid = WorkerSessionId::new("session-xyz");
        let cmd = DockerManager::claude_resume_command(
            &cid,
            &sid,
            WorkerRole::Member,
            Some("/home/agent/workspace"),
        );
        assert!(cmd.contains("--workdir /home/agent/workspace"));
        assert!(cmd.contains("--resume session-xyz"));
    }
}
