use super::TmuxManager;
use palette_domain::terminal::TerminalSessionName;

impl TmuxManager {
    /// Kill a tmux session. No-op if the session does not exist.
    pub fn kill_session(&self, name: &TerminalSessionName) -> crate::Result<()> {
        let name = name.as_ref();
        let output = self.run_tmux(&["kill-session", "-t", name])?;
        if output.status.success() {
            tracing::info!(session = name, "killed tmux session");
        } else {
            tracing::debug!(session = name, "tmux session not found, nothing to kill");
        }
        Ok(())
    }
}
