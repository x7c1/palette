mod helper;

use helper::{spawn_server, test_session_name_with_guard};
use palette_tmux::TmuxManager;

const TASK_TREE_YAML: &str = r#"
task:
  id: 2026/feature-x
  title: Add feature X

children:
  - id: planning
    children:
      - id: api-plan
        type: craft
        plan_path: 2026/feature-x/planning/api-plan
      - id: api-plan-review
        type: review
        depends_on: [api-plan]

  - id: execution
    depends_on: [planning]
    children:
      - id: api-impl
        type: craft
        plan_path: 2026/feature-x/execution/api-impl
      - id: api-impl-review
        type: review
        depends_on: [api-impl]
"#;

#[tokio::test]
async fn workflow_start_creates_task_tree() {
    let (session, _guard) = test_session_name_with_guard("wf-start");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();

    // POST /workflows/start
    let resp = client
        .post(format!("{base_url}/workflows/start"))
        .json(&serde_json::json!({
            "blueprint_yaml": TASK_TREE_YAML
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["workflow_id"].as_str().unwrap().starts_with("wf-"));
    // Root + planning + api-plan + api-plan-review + execution + api-impl + api-impl-review = 7
    assert_eq!(body["task_count"].as_u64().unwrap(), 7);

    // Verify tasks were created in the DB
    let root = state
        .db
        .get_task(&palette_domain::task::TaskId::new("2026/feature-x"))
        .unwrap();
    assert!(root.is_some());
    let root = root.unwrap();
    assert_eq!(root.title, "Add feature X");
    assert!(root.parent_id.is_none());

    // Check child tasks
    let children = state
        .db
        .get_child_tasks(&palette_domain::task::TaskId::new("2026/feature-x"))
        .unwrap();
    assert_eq!(children.len(), 2);

    let planning = children.iter().find(|t| t.title == "planning").unwrap();
    assert_eq!(
        planning.parent_id.as_ref().unwrap().to_string(),
        "2026/feature-x"
    );

    // Check grandchildren
    let planning_children = state.db.get_child_tasks(&planning.id).unwrap();
    assert_eq!(planning_children.len(), 2);

    // Verify initial task status resolution:
    // - root: InProgress (has children)
    // - planning: Ready (no dependencies)
    // - execution: Pending (depends on planning)
    // - api-plan: Ready (no dependencies within planning)
    // - api-plan-review: Pending (depends on api-plan)
    use palette_domain::task::TaskStatus;

    let root = state
        .db
        .get_task(&palette_domain::task::TaskId::new("2026/feature-x"))
        .unwrap()
        .unwrap();
    assert_eq!(root.status, TaskStatus::InProgress);

    let planning = state
        .db
        .get_task(&palette_domain::task::TaskId::new(
            "2026/feature-x/planning",
        ))
        .unwrap()
        .unwrap();
    assert_eq!(planning.status, TaskStatus::Ready);

    let execution = state
        .db
        .get_task(&palette_domain::task::TaskId::new(
            "2026/feature-x/execution",
        ))
        .unwrap()
        .unwrap();
    assert_eq!(execution.status, TaskStatus::Pending);

    let api_plan = state
        .db
        .get_task(&palette_domain::task::TaskId::new(
            "2026/feature-x/planning/api-plan",
        ))
        .unwrap()
        .unwrap();
    assert_eq!(api_plan.status, TaskStatus::Ready);

    let api_plan_review = state
        .db
        .get_task(&palette_domain::task::TaskId::new(
            "2026/feature-x/planning/api-plan-review",
        ))
        .unwrap()
        .unwrap();
    assert_eq!(api_plan_review.status, TaskStatus::Pending);
}

#[tokio::test]
async fn workflow_start_rejects_invalid_yaml() {
    let (session, _guard) = test_session_name_with_guard("wf-invalid");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, _state) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base_url}/workflows/start"))
        .json(&serde_json::json!({
            "blueprint_yaml": "not valid yaml: [["
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}
