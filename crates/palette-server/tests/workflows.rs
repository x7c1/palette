mod helper;

use helper::{spawn_server, test_session_name_with_guard};
use palette_domain::task::TaskStore;
use palette_tmux::TmuxManager;
use std::io::Write;

/// Write YAML to a temp file and return the path.
fn write_blueprint_file(yaml: &str) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(yaml.as_bytes()).unwrap();
    f
}

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
    let blueprint_file = write_blueprint_file(TASK_TREE_YAML);

    // POST /workflows/start
    let resp = client
        .post(format!("{base_url}/workflows/start"))
        .json(&serde_json::json!({
            "blueprint_path": blueprint_file.path().to_str().unwrap()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["workflow_id"].as_str().unwrap().starts_with("wf-"));
    // Root + planning + api-plan + api-plan-review + execution + api-impl + api-impl-review = 7
    assert_eq!(body["task_count"].as_u64().unwrap(), 7);

    // Verify tasks were created in the DB as a tree
    let root = state
        .db
        .get_task(&palette_domain::task::TaskId::new("2026/feature-x"))
        .unwrap()
        .unwrap();
    assert_eq!(root.title, "Add feature X");
    assert_eq!(root.children.len(), 2);

    let planning = root
        .children
        .iter()
        .find(|t| t.title == "planning")
        .unwrap();
    assert_eq!(planning.children.len(), 2);

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

    // Planning is a composite task: Ready → InProgress (has children)
    let planning = state
        .db
        .get_task(&palette_domain::task::TaskId::new(
            "2026/feature-x/planning",
        ))
        .unwrap()
        .unwrap();
    assert_eq!(planning.status, TaskStatus::InProgress);

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

    // Verify that a Job was created for the Ready craft task (api-plan)
    use palette_domain::job::JobFilter;
    let jobs = state
        .db
        .list_jobs(&JobFilter {
            job_type: None,
            status: None,
            assignee: None,
        })
        .unwrap();

    // Only api-plan should have a job (it's the only Ready leaf task with type: craft)
    assert_eq!(jobs.len(), 1);
    assert_eq!(
        jobs[0].task_id.as_ref().unwrap().to_string(),
        "2026/feature-x/planning/api-plan"
    );
    assert_eq!(jobs[0].job_type, palette_domain::job::JobType::Craft);
    // Craft jobs start as Draft then transition to Ready
    assert_eq!(jobs[0].status, palette_domain::job::JobStatus::Ready);
}

#[tokio::test]
async fn workflow_start_rejects_invalid_yaml() {
    let (session, _guard) = test_session_name_with_guard("wf-invalid");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, _state) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();
    let blueprint_file = write_blueprint_file("not valid yaml: [[");

    let resp = client
        .post(format!("{base_url}/workflows/start"))
        .json(&serde_json::json!({
            "blueprint_path": blueprint_file.path().to_str().unwrap()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn workflow_start_rejects_missing_file() {
    let (session, _guard) = test_session_name_with_guard("wf-missing");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, _state) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base_url}/workflows/start"))
        .json(&serde_json::json!({
            "blueprint_path": "/nonexistent/path.yaml"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

/// Test the full loop: Job Done → Task Done → sibling Task Ready → new Job
#[tokio::test]
async fn job_completion_cascades_through_task_tree() {
    let (session, _guard) = test_session_name_with_guard("wf-cascade");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();

    let yaml = r#"
task:
  id: 2026/cascade-test
  title: Cascade test

children:
  - id: step-a
    type: craft
    plan_path: test/step-a
  - id: step-b
    type: craft
    plan_path: test/step-b
    depends_on: [step-a]
"#;
    let blueprint_file = write_blueprint_file(yaml);

    let resp = client
        .post(format!("{base_url}/workflows/start"))
        .json(&serde_json::json!({
            "blueprint_path": blueprint_file.path().to_str().unwrap()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    use palette_domain::job::{JobFilter, JobStatus as JStatus, JobType};
    use palette_domain::task::{TaskId, TaskStatus};

    // step-a should have a Job in Ready state
    let jobs = state
        .db
        .list_jobs(&JobFilter {
            job_type: None,
            status: None,
            assignee: None,
        })
        .unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(
        jobs[0].task_id.as_ref().unwrap().to_string(),
        "2026/cascade-test/step-a"
    );

    // step-b should be Pending (depends on step-a)
    let step_b = state
        .db
        .get_task(&TaskId::new("2026/cascade-test/step-b"))
        .unwrap()
        .unwrap();
    assert_eq!(step_b.status, TaskStatus::Pending);

    // Simulate Job completion by sending a StatusChanged(Done) effect to the orchestrator.
    // This triggers propagate_task_completion inside process_effects.
    let job_id = &jobs[0].id;
    state
        .db
        .update_job_status(job_id, JStatus::InProgress)
        .unwrap();
    state
        .db
        .update_job_status(job_id, JStatus::InReview)
        .unwrap();
    state.db.update_job_status(job_id, JStatus::Done).unwrap();

    // Send StatusChanged effect to trigger orchestrator processing
    use palette_domain::rule::RuleEffect;
    use palette_domain::server::ServerEvent;
    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::StatusChanged {
            job_id: job_id.clone(),
            new_status: JStatus::Done,
        }],
    });

    // Give the orchestrator event loop time to process
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Verify: step-a task should be Done
    let step_a = state
        .db
        .get_task(&TaskId::new("2026/cascade-test/step-a"))
        .unwrap()
        .unwrap();
    assert_eq!(step_a.status, TaskStatus::Done);

    // Verify: step-b should now be Ready (dependency satisfied)
    let step_b = state
        .db
        .get_task(&TaskId::new("2026/cascade-test/step-b"))
        .unwrap()
        .unwrap();
    assert_eq!(step_b.status, TaskStatus::Ready);

    // Now complete step-b's job to trigger full workflow completion
    // step-b doesn't have a job yet (job creation for dynamically-readied tasks is pending)
    // So we create one manually and complete it
    let step_b_job = state
        .db
        .create_job(&palette_domain::job::CreateJobRequest {
            id: Some(palette_domain::job::JobId::new("C-step-b")),
            task_id: Some(TaskId::new("2026/cascade-test/step-b")),
            job_type: JobType::Craft,
            title: "step-b".to_string(),
            plan_path: "test/step-b".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repository: None,
            depends_on: vec![],
        })
        .unwrap();
    state
        .db
        .update_job_status(&step_b_job.id, JStatus::Ready)
        .unwrap();
    state
        .db
        .update_job_status(&step_b_job.id, JStatus::InProgress)
        .unwrap();
    state
        .db
        .update_job_status(&step_b_job.id, JStatus::InReview)
        .unwrap();
    state
        .db
        .update_job_status(&step_b_job.id, JStatus::Done)
        .unwrap();

    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::StatusChanged {
            job_id: step_b_job.id.clone(),
            new_status: JStatus::Done,
        }],
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // step-b task should be Done
    let step_b = state
        .db
        .get_task(&TaskId::new("2026/cascade-test/step-b"))
        .unwrap()
        .unwrap();
    assert_eq!(step_b.status, TaskStatus::Done);

    // Root task should be Done (all children complete)
    let root = state
        .db
        .get_task(&TaskId::new("2026/cascade-test"))
        .unwrap()
        .unwrap();
    assert_eq!(root.status, TaskStatus::Done);
}
