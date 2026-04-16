use palette_domain::terminal::{TerminalSessionName, TerminalTarget};

/// Port for terminal session management.
///
/// Abstracts tmux operations so that the orchestrator and server
/// can be tested with mock implementations.
pub trait TerminalSession: Send + Sync {
    fn create_session(
        &self,
        name: &TerminalSessionName,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn create_target(
        &self,
        name: &str,
    ) -> Result<TerminalTarget, Box<dyn std::error::Error + Send + Sync>>;

    fn create_pane(
        &self,
        base_target: &TerminalTarget,
    ) -> Result<TerminalTarget, Box<dyn std::error::Error + Send + Sync>>;

    fn send_keys(
        &self,
        target: &TerminalTarget,
        text: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn send_keys_no_enter(
        &self,
        target: &TerminalTarget,
        text: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn capture_pane(
        &self,
        target: &TerminalTarget,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>>;

    fn kill_session(
        &self,
        name: &TerminalSessionName,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}
