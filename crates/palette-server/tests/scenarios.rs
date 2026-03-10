mod common;

use common::{aid, capture_pane, create_work, spawn_server, test_session_name_with_guard, tid};
use palette_domain::agent::{AgentRole, AgentState, AgentStatus, ContainerId};
use palette_tmux::TmuxManager;
use serde_json::json;

/// Scenario 3: Multiple members stop while leader is working.
/// Event notifications are queued and delivered one at a time on each leader stop.
#[tokio::test]
async fn scenario3_message_queuing_to_leader() {
    let (session, _guard) = test_session_name_with_guard("scenario3");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let leader_pane = tmux.create_target("leader").unwrap();
    let _member_a_pane = tmux.create_target("member-a").unwrap();
    let _member_b_pane = tmux.create_target("member-b").unwrap();

    let (base_url, state) = spawn_server(tmux, &session).await;
    {
        let mut infra = state.infra.lock().await;
        infra.leaders.push(AgentState {
            id: aid("leader-1"),
            role: AgentRole::Leader,
            leader_id: aid(""),
            container_id: ContainerId::new(""),
            terminal_target: leader_pane.clone(),
            status: AgentStatus::Working,
            session_id: None,
        });
        infra.members.push(AgentState {
            id: aid("member-a"),
            role: AgentRole::Member,
            leader_id: aid("leader-1"),
            container_id: ContainerId::new(""),
            terminal_target: _member_a_pane.clone(),
            status: AgentStatus::Working,
            session_id: None,
        });
        infra.members.push(AgentState {
            id: aid("member-b"),
            role: AgentRole::Member,
            leader_id: aid("leader-1"),
            container_id: ContainerId::new(""),
            terminal_target: _member_b_pane.clone(),
            status: AgentStatus::Working,
            session_id: None,
        });
    }

    let client = reqwest::Client::new();

    // Create tasks and assign them (simulating auto-assign)
    client
        .post(format!("{base_url}/tasks/create"))
        .json(&create_work("W-A", "Task A"))
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base_url}/tasks/create"))
        .json(&create_work("W-B", "Task B"))
        .send()
        .await
        .unwrap();

    // Manually assign tasks (simulating what auto-assign does)
    state
        .db
        .update_task_status(&tid("W-A"), palette_domain::task::TaskStatus::Ready)
        .unwrap();
    state.db.assign_task(&tid("W-A"), &aid("member-a")).unwrap();
    state
        .db
        .update_task_status(&tid("W-B"), palette_domain::task::TaskStatus::Ready)
        .unwrap();
    state.db.assign_task(&tid("W-B"), &aid("member-b")).unwrap();

    // --- Both members stop while leader is Working ---

    // member-a stops
    let resp = client
        .post(format!("{base_url}/hooks/stop?member_id=member-a"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // member-b stops
    let resp = client
        .post(format!("{base_url}/hooks/stop?member_id=member-b"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Leader is Working, so both notifications should be queued
    assert!(
        state.db.has_pending_messages(&aid("leader-1")).unwrap(),
        "leader should have pending messages"
    );

    // Leader pane should NOT contain any review message yet (leader is Working)
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let content = capture_pane(&leader_pane);
    assert!(
        !content.contains("[review]"),
        "leader pane should not have reviews while Working, got: {content}"
    );

    // --- Leader stops (first time) → first queued message delivered ---
    let resp = client
        .post(format!("{base_url}/hooks/stop?member_id=leader-1"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let content = capture_pane(&leader_pane);
    assert!(
        content.contains("[review] task=W-A member=member-a"),
        "first stop should deliver member-a review, got: {content}"
    );

    // Leader should still have pending messages (member-b event)
    assert!(
        state.db.has_pending_messages(&aid("leader-1")).unwrap(),
        "leader should still have pending message for member-b"
    );

    // --- Leader stops (second time) → second queued message delivered ---
    let resp = client
        .post(format!("{base_url}/hooks/stop?member_id=leader-1"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let content = capture_pane(&leader_pane);
    assert!(
        content.contains("[review] task=W-B member=member-b"),
        "second stop should deliver member-b review, got: {content}"
    );

    // Queue should now be empty
    assert!(
        !state.db.has_pending_messages(&aid("leader-1")).unwrap(),
        "leader queue should be empty after all deliveries"
    );
}
