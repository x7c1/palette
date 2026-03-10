mod capture_pane;
mod create_pane;
mod create_session;
mod create_target;
mod is_session_alive;
mod is_terminal_alive;
mod send_keys;
mod send_keys_literal;
mod send_raw_key;

#[cfg(test)]
pub(super) mod testing;

use palette_domain::TerminalSessionName;
use std::process::Command;

pub struct TmuxManager {
    session_name: TerminalSessionName,
}

impl TmuxManager {
    pub fn new(session_name: TerminalSessionName) -> Self {
        Self { session_name }
    }

    pub(super) fn run_tmux(&self, args: &[&str]) -> crate::Result<std::process::Output> {
        Ok(Command::new("tmux").args(args).output()?)
    }
}
