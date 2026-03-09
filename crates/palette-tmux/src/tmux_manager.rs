pub trait TerminalManager {
    fn create_session(&self, name: &str) -> crate::Result<()>;
    fn create_target(&self, name: &str) -> crate::Result<String>;
    fn create_pane(&self, base_target: &str) -> crate::Result<String>;
    fn send_keys(&self, target: &str, text: &str) -> crate::Result<()>;
    /// Send text without appending Enter key. Used for permission prompt responses.
    fn send_keys_literal(&self, target: &str, text: &str) -> crate::Result<()>;
    fn send_raw_key(&self, target: &str, key: &str) -> crate::Result<()>;
    fn capture_pane(&self, target: &str) -> crate::Result<String>;
    fn is_alive(&self, target: &str) -> crate::Result<bool>;
}
