use super::TmuxManager;
use crate::Error;
use palette_domain::terminal::TerminalTarget;

impl TmuxManager {
    pub fn create_target(&self, name: &str) -> crate::Result<TerminalTarget> {
        let session = self.session_name.as_ref();
        let target = format!("{session}:{name}");

        // Create a new window with the given name
        let output = self.run_tmux(&[
            "new-window",
            "-t",
            session,
            "-n",
            name,
            "-P",
            "-F",
            "#{pane_id}",
        ])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to create tmux target '{target}': {stderr}"
            )));
        }

        let pane_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        tracing::info!(target = %target, pane_id = %pane_id, "created tmux target");
        // Return pane_id for precise targeting (window name is ambiguous with multiple panes)
        Ok(TerminalTarget::new(pane_id))
    }
}
