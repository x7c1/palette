use super::TmuxManager;
use palette_domain::terminal::TerminalSessionName;

impl TmuxManager {
    pub fn is_session_alive(&self, name: &TerminalSessionName) -> crate::Result<bool> {
        let output = self.run_tmux(&["has-session", "-t", name.as_ref()])?;
        Ok(output.status.success())
    }
}

#[cfg(test)]
mod tests {
    use super::TmuxManager;
    use palette_domain::terminal::TerminalSessionName;

    #[test]
    fn returns_false_for_nonexistent() {
        let session = TerminalSessionName::new("nonexistent-session-12345");
        let tmux = TmuxManager::new(session.clone());
        assert!(!tmux.is_session_alive(&session).unwrap());
    }
}
