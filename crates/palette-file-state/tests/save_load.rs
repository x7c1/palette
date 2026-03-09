use palette_domain::{
    AgentId, AgentRole, AgentState, AgentStatus, ContainerId, PersistentState, TerminalTarget,
};

fn aid(s: &str) -> AgentId {
    AgentId::new(s)
}

#[test]
fn save_and_load_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");

    let mut state = PersistentState::new("test-session".to_string());
    state.members.push(AgentState {
        id: aid("member-a"),
        role: AgentRole::Member,
        leader_id: aid("leader-1"),
        container_id: ContainerId::new("abc123"),
        terminal_target: TerminalTarget::new("test-session:member-a"),
        status: AgentStatus::Idle,
        session_id: None,
    });

    palette_file_state::save(&state, &path).unwrap();
    let loaded = palette_file_state::load(&path).unwrap().unwrap();
    assert_eq!(loaded.session_name, "test-session");
    assert_eq!(loaded.members.len(), 1);
    assert_eq!(loaded.members[0].id, aid("member-a"));
    assert_eq!(loaded.members[0].role, AgentRole::Member);
    assert_eq!(loaded.members[0].status, AgentStatus::Idle);
    assert_eq!(loaded.members[0].container_id.as_ref(), "abc123");
}

#[test]
fn load_nonexistent_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nope.json");
    assert!(palette_file_state::load(&path).unwrap().is_none());
}

#[test]
fn atomic_save_leaves_no_tmp_on_success() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    let tmp_path = dir.path().join("state.json.tmp");

    let state = PersistentState::new("test".to_string());
    palette_file_state::save(&state, &path).unwrap();

    assert!(path.exists());
    assert!(!tmp_path.exists());
}
