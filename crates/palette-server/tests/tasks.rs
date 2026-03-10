mod helper;

use helper::{
    create_review, create_work, spawn_server, test_session_name_with_guard, update_status,
};
use palette_server::api_types::{CreateTaskRequest, TaskStatus, TaskType};
use palette_tmux::TmuxManager;

#[tokio::test]
async fn task_api_create_and_list() {
    let (session, _guard) = test_session_name_with_guard("taskapi");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, _state) = spawn_server(tmux, &session).await;

    let client = reqwest::Client::new();

    // Create a work task
    let resp = client
        .post(format!("{base_url}/tasks/create"))
        .json(&CreateTaskRequest {
            id: Some("W-001".to_string()),
            task_type: TaskType::Work,
            title: "Implement feature".to_string(),
            description: Some("Details here".to_string()),
            assignee: Some("member-a".to_string()),
            priority: Some(palette_server::api_types::Priority::High),
            repositories: None,
            depends_on: vec![],
        })
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let task: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(task["id"], "W-001");
    assert_eq!(task["status"], "draft");

    // Create a review task depending on W-001
    let resp = client
        .post(format!("{base_url}/tasks/create"))
        .json(&CreateTaskRequest {
            id: Some("R-001".to_string()),
            task_type: TaskType::Review,
            title: "Review feature".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: vec!["W-001".to_string()],
        })
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let review: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(review["status"], "todo");

    // List all tasks
    let tasks: Vec<serde_json::Value> = client
        .get(format!("{base_url}/tasks"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(tasks.len(), 2);

    // List work tasks only
    let tasks: Vec<serde_json::Value> = client
        .get(format!("{base_url}/tasks?type=work"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(tasks.len(), 1);
}

#[tokio::test]
async fn task_api_update_with_rules() {
    let (session, _guard) = test_session_name_with_guard("taskrules");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, _state) = spawn_server(tmux, &session).await;

    let client = reqwest::Client::new();

    // Create work + review
    client
        .post(format!("{base_url}/tasks/create"))
        .json(&create_work("W-001", "Work"))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{base_url}/tasks/create"))
        .json(&create_review("R-001", "Review", vec!["W-001"]))
        .send()
        .await
        .unwrap();

    // Transition W-001: draft -> ready -> in_progress -> in_review
    client
        .post(format!("{base_url}/tasks/update"))
        .json(&update_status("W-001", TaskStatus::Ready))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{base_url}/tasks/update"))
        .json(&update_status("W-001", TaskStatus::InProgress))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{base_url}/tasks/update"))
        .json(&update_status("W-001", TaskStatus::InReview))
        .send()
        .await
        .unwrap();

    // Invalid transition should fail (in_review -> draft)
    let resp = client
        .post(format!("{base_url}/tasks/update"))
        .json(&update_status("W-001", TaskStatus::Draft))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}
