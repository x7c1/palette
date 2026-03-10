use super::TmuxManager;
use crate::Error;
use palette_domain::terminal::TerminalSessionName;

impl TmuxManager {
    pub fn create_session(&self, name: &TerminalSessionName) -> crate::Result<()> {
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
}

#[cfg(test)]
mod tests {
    use super::super::testing;

    #[test]
    fn create_session_and_target() {
        let (tmux, session) = testing::setup("create");

        tmux.create_session(&session).unwrap();
        assert!(tmux.is_session_alive(&session).unwrap());

        let target = tmux.create_target("test-pane").unwrap();
        assert!(tmux.is_terminal_alive(&target).unwrap());

        testing::cleanup_session(&session);
    }

    #[test]
    fn create_session_idempotent() {
        let (tmux, session) = testing::setup("idempotent");

        tmux.create_session(&session).unwrap();
        tmux.create_session(&session).unwrap(); // should not fail
        assert!(tmux.is_session_alive(&session).unwrap());

        testing::cleanup_session(&session);
    }
}
