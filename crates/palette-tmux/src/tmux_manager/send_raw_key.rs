use super::TmuxManager;
use crate::Error;
use palette_domain::terminal::TerminalTarget;

impl TmuxManager {
    pub fn send_raw_key(&self, target: &TerminalTarget, key: &str) -> crate::Result<()> {
        let output = self.run_tmux(&["send-keys", "-t", target.as_ref(), key])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to send raw key to '{target}': {stderr}"
            )));
        }
        Ok(())
    }
}
