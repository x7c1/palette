use palette_domain::terminal::{TerminalSessionName, TerminalTarget};
use palette_usecase::TerminalSession;
use std::collections::HashMap;
use std::sync::Mutex;

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

pub struct MockTerminalSession {
    pub pane_content: Mutex<HashMap<String, String>>,
    pub sent_keys: Mutex<Vec<(String, String)>>,
}

impl MockTerminalSession {
    pub fn new() -> Self {
        Self {
            pane_content: Mutex::new(HashMap::new()),
            sent_keys: Mutex::new(Vec::new()),
        }
    }

    pub fn set_pane_content(&self, target: &str, content: &str) {
        self.pane_content
            .lock()
            .unwrap()
            .insert(target.to_string(), content.to_string());
    }
}

impl TerminalSession for MockTerminalSession {
    fn capture_pane(&self, target: &TerminalTarget) -> Result<String, BoxErr> {
        Ok(self
            .pane_content
            .lock()
            .unwrap()
            .get(target.as_ref())
            .cloned()
            .unwrap_or_default())
    }

    fn send_keys(&self, target: &TerminalTarget, text: &str) -> Result<(), BoxErr> {
        self.sent_keys
            .lock()
            .unwrap()
            .push((target.as_ref().to_string(), text.to_string()));
        Ok(())
    }

    fn send_keys_no_enter(&self, target: &TerminalTarget, text: &str) -> Result<(), BoxErr> {
        self.sent_keys
            .lock()
            .unwrap()
            .push((target.as_ref().to_string(), text.to_string()));
        Ok(())
    }

    fn create_target(&self, _: &str) -> Result<TerminalTarget, BoxErr> {
        unimplemented!()
    }
    fn create_pane(&self, _: &TerminalTarget) -> Result<TerminalTarget, BoxErr> {
        unimplemented!()
    }
    fn kill_session(&self, _: &TerminalSessionName) -> Result<(), BoxErr> {
        unimplemented!()
    }
}
