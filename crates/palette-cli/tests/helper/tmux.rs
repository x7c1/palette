use super::SessionGuard;
use palette_domain::terminal::{TerminalSessionName, TerminalTarget};
use std::process::Command;

/// Unique session name for each test to avoid conflicts.
pub fn test_session_name(test_name: &str) -> TerminalSessionName {
    TerminalSessionName::new(format!("palette-test-{}-{}", test_name, std::process::id()))
}

/// Create a session name and a guard that cleans up the tmux session on drop.
pub fn test_session_name_with_guard(test_name: &str) -> (TerminalSessionName, SessionGuard) {
    let name = test_session_name(test_name);
    let guard = SessionGuard::new(name.clone());
    (name, guard)
}

/// Capture the content of a tmux pane (including scrollback buffer).
pub fn capture_pane(target: &TerminalTarget) -> String {
    let output = Command::new("tmux")
        .args([
            "capture-pane",
            "-t",
            target.as_ref(),
            "-p",
            "-J",
            "-S",
            "-200",
        ])
        .output()
        .expect("failed to capture pane");
    String::from_utf8_lossy(&output.stdout).to_string()
}
