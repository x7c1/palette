use palette_domain::terminal::TerminalSessionName;
use std::process::Command;

/// RAII guard that kills the tmux session on drop (including panic).
pub struct SessionGuard(TerminalSessionName);

impl SessionGuard {
    pub fn new(session: TerminalSessionName) -> Self {
        Self(session)
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", self.0.as_ref()])
            .output();
    }
}
