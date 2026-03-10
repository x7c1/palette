use super::TmuxManager;
use crate::Error;
use palette_domain::terminal::TerminalTarget;

impl TmuxManager {
    pub fn capture_pane(&self, target: &TerminalTarget) -> crate::Result<String> {
        let output = self.run_tmux(&["capture-pane", "-t", target.as_ref(), "-p", "-J"])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to capture pane '{target}': {stderr}"
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
