use super::TmuxManager;
use crate::Error;
use palette_domain::terminal::TerminalTarget;

impl TmuxManager {
    pub fn send_keys(&self, target: &TerminalTarget, text: &str) -> crate::Result<()> {
        // Use literal mode (-l) to avoid interpretation of special characters
        let output = self.run_tmux(&["send-keys", "-t", target.as_ref(), "-l", text])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to send keys to '{target}': {stderr}"
            )));
        }

        // Send Enter key separately (not in literal mode)
        let output = self.run_tmux(&["send-keys", "-t", target.as_ref(), "Enter"])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to send Enter to '{target}': {stderr}"
            )));
        }

        // Wait briefly for the terminal to process, then check for bracketed paste
        std::thread::sleep(std::time::Duration::from_millis(500));
        if let Ok(pane) = self.capture_pane(target)
            && pane.contains("[Pasted text")
        {
            tracing::info!(target = %target, "bracketed paste detected, sending extra Enter");
            let _ = self.send_raw_key(target, "Enter");
        }

        tracing::debug!(target = %target, "sent keys");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing;

    #[test]
    fn preserves_special_characters() {
        let (tmux, session) = testing::setup("literal");

        tmux.create_session(&session).unwrap();
        let target = tmux.create_target("worker").unwrap();

        // Send a command with special characters
        tmux.send_keys(&target, r#"echo "test; value""#).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(300));

        let content = tmux.capture_pane(&target).unwrap();
        assert!(
            content.contains("test; value"),
            "expected special chars preserved, got: {content}"
        );

        testing::cleanup_session(&session);
    }
}
