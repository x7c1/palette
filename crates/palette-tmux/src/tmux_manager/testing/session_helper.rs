use super::super::TmuxManager;
use palette_domain::terminal::TerminalSessionName;

pub fn test_session_name(test_name: &str) -> TerminalSessionName {
    TerminalSessionName::new(format!(
        "palette-tmux-test-{}-{}",
        test_name,
        std::process::id()
    ))
}

pub fn cleanup_session(session: &TerminalSessionName) {
    let _ = std::process::Command::new("tmux")
        .args(["kill-session", "-t", session.as_ref()])
        .output();
}

pub fn setup(test_name: &str) -> (TmuxManager, TerminalSessionName) {
    let session = test_session_name(test_name);
    let tmux = TmuxManager::new(session.clone());
    (tmux, session)
}
