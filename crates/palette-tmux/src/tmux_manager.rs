pub trait TmuxManager {
    fn create_session(&self, name: &str) -> anyhow::Result<()>;
    fn create_target(&self, name: &str) -> anyhow::Result<String>;
    fn create_pane(&self, base_target: &str) -> anyhow::Result<String>;
    fn send_keys(&self, target: &str, text: &str) -> anyhow::Result<()>;
    /// Send text without appending Enter key. Used for permission prompt responses.
    fn send_keys_literal(&self, target: &str, text: &str) -> anyhow::Result<()>;
    fn send_raw_key(&self, target: &str, key: &str) -> anyhow::Result<()>;
    fn capture_pane(&self, target: &str) -> anyhow::Result<String>;
    fn is_alive(&self, target: &str) -> anyhow::Result<bool>;
}
