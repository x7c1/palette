use super::TmuxManager;
use crate::Error;
use palette_domain::terminal::TerminalTarget;

impl TmuxManager {
    pub fn send_keys_no_enter(&self, target: &TerminalTarget, text: &str) -> crate::Result<()> {
        let output = self.run_tmux(&["send-keys", "-t", target.as_ref(), "-l", text])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to send literal keys to '{target}': {stderr}"
            )));
        }
        tracing::debug!(target = %target, "sent literal keys (no enter)");
        Ok(())
    }
}
