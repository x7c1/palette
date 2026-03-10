use super::TmuxManager;
use palette_domain::terminal::TerminalTarget;

impl TmuxManager {
    pub fn is_terminal_alive(&self, target: &TerminalTarget) -> crate::Result<bool> {
        let output = self.run_tmux(&["has-session", "-t", target.as_ref()])?;
        Ok(output.status.success())
    }
}
