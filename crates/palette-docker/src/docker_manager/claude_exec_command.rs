use super::DockerManager;
use palette_domain::worker::{ContainerId, WorkerRole};

impl DockerManager {
    /// Build the command string to launch Claude Code inside a container's tmux pane.
    /// Leaders bypass permissions (they run in a sandbox container).
    /// Members keep default permissions (the leader handles their permission prompts).
    pub fn claude_exec_command(
        container_id: &ContainerId,
        prompt_file: &str,
        role: WorkerRole,
    ) -> String {
        let cid = container_id.as_ref();
        let plugin_flag = " --plugin-dir /home/agent/claude-code-plugin";
        if role.is_supervisor() {
            format!(
                "docker exec -it {cid} claude --dangerously-skip-permissions --append-system-prompt-file {prompt_file}{plugin_flag}"
            )
        } else {
            format!(
                "docker exec -it {cid} claude --append-system-prompt-file {prompt_file}{plugin_flag}"
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::DockerManager;
    use palette_domain::worker::{ContainerId, WorkerRole};

    #[test]
    fn leader_bypasses_permissions() {
        let cid = ContainerId::new("abc123");
        let cmd = DockerManager::claude_exec_command(
            &cid,
            "/home/agent/prompts/leader.md",
            WorkerRole::Leader,
        );
        assert!(cmd.contains("docker exec -it abc123 claude"));
        assert!(cmd.contains("--dangerously-skip-permissions"));
        assert!(cmd.contains("--append-system-prompt-file /home/agent/prompts/leader.md"));
        assert!(cmd.contains("--plugin-dir /home/agent/claude-code-plugin"));
    }

    #[test]
    fn member_keeps_permissions() {
        let cid = ContainerId::new("abc123");
        let cmd = DockerManager::claude_exec_command(
            &cid,
            "/home/agent/prompts/member.md",
            WorkerRole::Member,
        );
        assert!(cmd.contains("docker exec -it abc123 claude"));
        assert!(!cmd.contains("--dangerously-skip-permissions"));
        assert!(cmd.contains("--append-system-prompt-file /home/agent/prompts/member.md"));
        assert!(cmd.contains("--plugin-dir /home/agent/claude-code-plugin"));
    }

    #[test]
    fn review_integrator_bypasses_permissions() {
        let cid = ContainerId::new("abc123");
        let cmd = DockerManager::claude_exec_command(
            &cid,
            "/home/agent/prompts/review-integrator.md",
            WorkerRole::ReviewIntegrator,
        );
        assert!(cmd.contains("docker exec -it abc123 claude"));
        assert!(cmd.contains("--dangerously-skip-permissions"));
        assert!(
            cmd.contains("--append-system-prompt-file /home/agent/prompts/review-integrator.md")
        );
    }
}
