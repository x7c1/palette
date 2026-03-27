use crate::TmuxManager;
use palette_domain::terminal::{TerminalSessionName, TerminalTarget};
use palette_usecase::TerminalSession;

impl TerminalSession for TmuxManager {
    fn create_target(
        &self,
        name: &str,
    ) -> Result<TerminalTarget, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.create_target(name)?)
    }

    fn create_pane(
        &self,
        base_target: &TerminalTarget,
    ) -> Result<TerminalTarget, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.create_pane(base_target)?)
    }

    fn send_keys(
        &self,
        target: &TerminalTarget,
        text: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.send_keys(target, text)?)
    }

    fn send_keys_no_enter(
        &self,
        target: &TerminalTarget,
        text: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.send_keys_no_enter(target, text)?)
    }

    fn capture_pane(
        &self,
        target: &TerminalTarget,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.capture_pane(target)?)
    }

    fn kill_session(
        &self,
        name: &TerminalSessionName,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.kill_session(name)?)
    }
}
