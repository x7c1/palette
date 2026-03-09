use crate::Error;
use std::process::Command;

use crate::TerminalManager;

pub struct TmuxManagerImpl {
    session_name: String,
}

impl TmuxManagerImpl {
    pub fn new(session_name: String) -> Self {
        Self { session_name }
    }

    fn run_tmux(&self, args: &[&str]) -> crate::Result<std::process::Output> {
        Ok(Command::new("tmux").args(args).output()?)
    }
}

impl TerminalManager for TmuxManagerImpl {
    fn create_session(&self, name: &str) -> crate::Result<()> {
        let output = self.run_tmux(&["has-session", "-t", name])?;
        if output.status.success() {
            tracing::info!(session = name, "tmux session already exists");
            return Ok(());
        }
        let output = self.run_tmux(&["new-session", "-d", "-s", name, "-x", "200", "-y", "50"])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to create tmux session '{name}': {stderr}"
            )));
        }
        tracing::info!(session = name, "created tmux session");
        Ok(())
    }

    fn create_target(&self, name: &str) -> crate::Result<String> {
        let target = format!("{}:{}", self.session_name, name);

        // Create a new window with the given name
        let output = self.run_tmux(&[
            "new-window",
            "-t",
            &self.session_name,
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
        Ok(pane_id)
    }

    fn create_pane(&self, base_target: &str) -> crate::Result<String> {
        // Split the base target horizontally to create a side-by-side pane
        let output = self.run_tmux(&[
            "split-window",
            "-h",
            "-t",
            base_target,
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
        Ok(pane_id)
    }

    fn send_keys(&self, target: &str, text: &str) -> crate::Result<()> {
        // Use literal mode (-l) to avoid interpretation of special characters
        let output = self.run_tmux(&["send-keys", "-t", target, "-l", text])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to send keys to '{target}': {stderr}"
            )));
        }

        // Send Enter key separately (not in literal mode)
        let output = self.run_tmux(&["send-keys", "-t", target, "Enter"])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to send Enter to '{target}': {stderr}"
            )));
        }

        tracing::debug!(target = target, "sent keys");
        Ok(())
    }

    fn send_keys_literal(&self, target: &str, text: &str) -> crate::Result<()> {
        let output = self.run_tmux(&["send-keys", "-t", target, "-l", text])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to send literal keys to '{target}': {stderr}"
            )));
        }
        tracing::debug!(target = target, "sent literal keys (no enter)");
        Ok(())
    }

    fn send_raw_key(&self, target: &str, key: &str) -> crate::Result<()> {
        let output = self.run_tmux(&["send-keys", "-t", target, key])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to send raw key to '{target}': {stderr}"
            )));
        }
        Ok(())
    }

    fn capture_pane(&self, target: &str) -> crate::Result<String> {
        let output = self.run_tmux(&["capture-pane", "-t", target, "-p", "-J"])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to capture pane '{target}': {stderr}"
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn is_alive(&self, target: &str) -> crate::Result<bool> {
        let output = self.run_tmux(&["has-session", "-t", target])?;
        Ok(output.status.success())
    }
}
