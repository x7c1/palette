use crate::Error;
use palette_domain::{TerminalSessionName, TerminalTarget};
use std::process::Command;

use crate::TerminalManager;

pub struct TmuxManagerImpl {
    session_name: TerminalSessionName,
}

impl TmuxManagerImpl {
    pub fn new(session_name: TerminalSessionName) -> Self {
        Self { session_name }
    }

    fn run_tmux(&self, args: &[&str]) -> crate::Result<std::process::Output> {
        Ok(Command::new("tmux").args(args).output()?)
    }
}

impl TerminalManager for TmuxManagerImpl {
    fn create_session(&self, name: &TerminalSessionName) -> crate::Result<()> {
        let name = name.as_ref();
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

    fn create_target(&self, name: &str) -> crate::Result<TerminalTarget> {
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

    fn create_pane(&self, base_target: &TerminalTarget) -> crate::Result<TerminalTarget> {
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
        Ok(TerminalTarget::new(pane_id))
    }

    fn send_keys(&self, target: &TerminalTarget, text: &str) -> crate::Result<()> {
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

        tracing::debug!(target = %target, "sent keys");
        Ok(())
    }

    fn send_keys_literal(&self, target: &TerminalTarget, text: &str) -> crate::Result<()> {
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

    fn send_raw_key(&self, target: &TerminalTarget, key: &str) -> crate::Result<()> {
        let output = self.run_tmux(&["send-keys", "-t", target.as_ref(), key])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to send raw key to '{target}': {stderr}"
            )));
        }
        Ok(())
    }

    fn capture_pane(&self, target: &TerminalTarget) -> crate::Result<String> {
        let output = self.run_tmux(&["capture-pane", "-t", target.as_ref(), "-p", "-J"])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(format!(
                "failed to capture pane '{target}': {stderr}"
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn is_session_alive(&self, name: &TerminalSessionName) -> crate::Result<bool> {
        let output = self.run_tmux(&["has-session", "-t", name.as_ref()])?;
        Ok(output.status.success())
    }

    fn is_terminal_alive(&self, target: &TerminalTarget) -> crate::Result<bool> {
        let output = self.run_tmux(&["has-session", "-t", target.as_ref()])?;
        Ok(output.status.success())
    }
}
