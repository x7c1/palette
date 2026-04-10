use super::{BoxErr, unsupported};
use palette_domain::terminal::{TerminalSessionName, TerminalTarget};
use palette_usecase::TerminalSession;

pub(in crate::admin) struct NoopTerminal;

impl TerminalSession for NoopTerminal {
    fn create_target(&self, _name: &str) -> Result<TerminalTarget, BoxErr> {
        unsupported("create_target")
    }

    fn create_pane(&self, _base_target: &TerminalTarget) -> Result<TerminalTarget, BoxErr> {
        unsupported("create_pane")
    }

    fn send_keys(&self, _target: &TerminalTarget, _text: &str) -> Result<(), BoxErr> {
        unsupported("send_keys")
    }

    fn send_keys_no_enter(&self, _target: &TerminalTarget, _text: &str) -> Result<(), BoxErr> {
        unsupported("send_keys_no_enter")
    }

    fn capture_pane(&self, _target: &TerminalTarget) -> Result<String, BoxErr> {
        unsupported("capture_pane")
    }

    fn kill_session(&self, _name: &TerminalSessionName) -> Result<(), BoxErr> {
        unsupported("kill_session")
    }
}
