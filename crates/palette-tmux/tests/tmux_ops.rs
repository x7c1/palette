use palette_tmux::{TerminalManager, TmuxManagerImpl};
use std::process::Command;

fn test_session_name(test_name: &str) -> String {
    format!("palette-tmux-test-{}-{}", test_name, std::process::id())
}

fn cleanup_session(session: &str) {
    let _ = Command::new("tmux")
        .args(["kill-session", "-t", session])
        .output();
}

#[test]
fn create_session_and_target() {
    let session = test_session_name("create");
    let tmux = TmuxManagerImpl::new(session.clone());

    tmux.create_session(&session).unwrap();
    assert!(tmux.is_alive(&session).unwrap());

    let target = tmux.create_target("test-pane").unwrap();
    assert!(tmux.is_alive(target.as_ref()).unwrap());

    cleanup_session(&session);
}

#[test]
fn create_session_idempotent() {
    let session = test_session_name("idempotent");
    let tmux = TmuxManagerImpl::new(session.clone());

    tmux.create_session(&session).unwrap();
    tmux.create_session(&session).unwrap(); // should not fail
    assert!(tmux.is_alive(&session).unwrap());

    cleanup_session(&session);
}

#[test]
fn is_alive_returns_false_for_nonexistent() {
    let tmux = TmuxManagerImpl::new("nonexistent".to_string());
    assert!(!tmux.is_alive("nonexistent-session-12345").unwrap());
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
