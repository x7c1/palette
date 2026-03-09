use palette_domain::TerminalSessionName;
use palette_tmux::{TerminalManager, TmuxManagerImpl};
use std::process::Command;

fn test_session_name(test_name: &str) -> TerminalSessionName {
    TerminalSessionName::new(format!(
        "palette-tmux-test-{}-{}",
        test_name,
        std::process::id()
    ))
}

fn cleanup_session(session: &TerminalSessionName) {
    let _ = Command::new("tmux")
        .args(["kill-session", "-t", session.as_ref()])
        .output();
}

#[test]
fn create_session_and_target() {
    let session = test_session_name("create");
    let tmux = TmuxManagerImpl::new(session.clone());

    tmux.create_session(&session).unwrap();
    assert!(tmux.is_session_alive(&session).unwrap());

    let target = tmux.create_target("test-pane").unwrap();
    assert!(tmux.is_terminal_alive(&target).unwrap());

    cleanup_session(&session);
}

#[test]
fn create_session_idempotent() {
    let session = test_session_name("idempotent");
    let tmux = TmuxManagerImpl::new(session.clone());

    tmux.create_session(&session).unwrap();
    tmux.create_session(&session).unwrap(); // should not fail
    assert!(tmux.is_session_alive(&session).unwrap());

    cleanup_session(&session);
}

#[test]
fn is_alive_returns_false_for_nonexistent() {
    let session = TerminalSessionName::new("nonexistent-session-12345");
    let tmux = TmuxManagerImpl::new(session.clone());
    assert!(!tmux.is_session_alive(&session).unwrap());
}

#[test]
fn send_keys_literal_mode() {
    let session = test_session_name("literal");
    let tmux = TmuxManagerImpl::new(session.clone());

    tmux.create_session(&session).unwrap();
    let target = tmux.create_target("worker").unwrap();

    // Send a command with special characters
    tmux.send_keys(&target, r#"echo "test; value""#).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(300));

    let output = Command::new("tmux")
        .args(["capture-pane", "-t", target.as_ref(), "-p"])
        .output()
        .unwrap();
    let content = String::from_utf8_lossy(&output.stdout);
    assert!(
        content.contains("test; value"),
        "expected special chars preserved, got: {content}"
    );

    cleanup_session(&session);
}
