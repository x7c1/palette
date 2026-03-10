mod helper;

use helper::{aid, capture_pane, spawn_server, test_session_name_with_guard, tid};
use palette_domain::agent::{AgentRole, AgentState, AgentStatus, ContainerId};
use palette_domain::task::{CreateTaskRequest, TaskStatus, TaskType};
use palette_tmux::TmuxManager;
use serde_json::json;

/// Scenario 3: Multiple review members stop while review integrator is working.
/// Event notifications are queued and delivered one at a time on each leader stop.
#[tokio::test]
async fn scenario3_message_queuing_to_leader() {
    let (session, _guard) = test_session_name_with_guard("scenario3");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let ri_pane = tmux.create_target("review-integrator").unwrap();
    let _member_a_pane = tmux.create_target("member-a").unwrap();
    let _member_b_pane = tmux.create_target("member-b").unwrap();

    let (base_url, state) = spawn_server(tmux, &session).await;
    {
        let mut infra = state.infra.lock().await;
        infra.leaders.push(AgentState {
            id: aid("review-integrator-1"),
            role: AgentRole::ReviewIntegrator,
            leader_id: aid(""),
            container_id: ContainerId::new(""),
            terminal_target: ri_pane.clone(),
            status: AgentStatus::Working,
            session_id: None,
        });
        infra.members.push(AgentState {
            id: aid("member-a"),
            role: AgentRole::Member,
            leader_id: aid("review-integrator-1"),
            container_id: ContainerId::new(""),
            terminal_target: _member_a_pane.clone(),
            status: AgentStatus::Working,
            session_id: None,
        });
        infra.members.push(AgentState {
            id: aid("member-b"),
            role: AgentRole::Member,
            leader_id: aid("review-integrator-1"),
            container_id: ContainerId::new(""),
            terminal_target: _member_b_pane.clone(),
            status: AgentStatus::Working,
            session_id: None,
        });
    }

    let client = reqwest::Client::new();

    // Create review tasks and assign them
    state
        .db
        .create_task(&CreateTaskRequest {
            id: Some(tid("R-A")),
            task_type: TaskType::Review,
            title: "Review A".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![],
        })
        .unwrap();
    state
        .db
        .create_task(&CreateTaskRequest {
            id: Some(tid("R-B")),
            task_type: TaskType::Review,
            title: "Review B".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec![],
        })
        .unwrap();

    state
        .db
        .update_task_status(&tid("R-A"), TaskStatus::Ready)
        .unwrap();
    state.db.assign_task(&tid("R-A"), &aid("member-a")).unwrap();
    state
        .db
        .update_task_status(&tid("R-B"), TaskStatus::Ready)
        .unwrap();
    state.db.assign_task(&tid("R-B"), &aid("member-b")).unwrap();

    // --- Both review members stop while review integrator is Working ---

    // member-a stops
    let resp = client
        .post(format!("{base_url}/hooks/stop?member_id=member-a"))
        .json(&json!({"last_assistant_message": "review findings A"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // member-b stops
    let resp = client
        .post(format!("{base_url}/hooks/stop?member_id=member-b"))
        .json(&json!({"last_assistant_message": "review findings B"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Review integrator is Working, so both notifications should be queued
    assert!(
        state
            .db
            .has_pending_messages(&aid("review-integrator-1"))
            .unwrap(),
        "review integrator should have pending messages"
    );

    // RI pane should NOT contain any review message yet (RI is Working)
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let content = capture_pane(&ri_pane);
    assert!(
        !content.contains("[review]"),
        "RI pane should not have reviews while Working, got: {content}"
    );

    // --- RI stops (first time) → first queued message delivered ---
    let resp = client
        .post(format!(
            "{base_url}/hooks/stop?member_id=review-integrator-1"
        ))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let content = capture_pane(&ri_pane);
    assert!(
        content.contains("[review] member=member-a task=R-A type=review_complete"),
        "first stop should deliver member-a review, got: {content}"
    );

    // RI should still have pending messages (member-b event)
    assert!(
        state
            .db
            .has_pending_messages(&aid("review-integrator-1"))
            .unwrap(),
        "RI should still have pending message for member-b"
    );

    // --- RI stops (second time) → second queued message delivered ---
    let resp = client
        .post(format!(
            "{base_url}/hooks/stop?member_id=review-integrator-1"
        ))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let content = capture_pane(&ri_pane);
    assert!(
        content.contains("[review] member=member-b task=R-B type=review_complete"),
        "second stop should deliver member-b review, got: {content}"
    );

    // Queue should now be empty
    assert!(
        !state
            .db
            .has_pending_messages(&aid("review-integrator-1"))
            .unwrap(),
        "RI queue should be empty after all deliveries"
    );
}
