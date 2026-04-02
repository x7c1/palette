use super::TmuxManager;
use crate::Error;
use palette_domain::terminal::TerminalTarget;

impl TmuxManager {
    pub fn create_pane(&self, base_target: &TerminalTarget) -> crate::Result<TerminalTarget> {
        // Split the base target horizontally to create a side-by-side pane
        let output = self.run_tmux(&[
            "split-window",
            "-h",
            "-t",
            base_target.as_ref(),
            "-P",
            "-F",
            "#{pane_id}",
        ])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to split pane at '{base_target}': {stderr}"
            )));
        }

        let pane_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        tracing::info!(base_target = %base_target, pane_id = %pane_id, "created tmux pane");

        // Re-balance all panes in the window to equal widths.
        // Without this, each split-window halves the current pane,
        // making later panes progressively narrower.
        let _ = self.run_tmux(&["select-layout", "-t", base_target.as_ref(), "even-horizontal"]);

        Ok(TerminalTarget::new(pane_id))
    }
}
