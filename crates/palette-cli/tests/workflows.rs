mod helper;

use helper::{spawn_server, test_session_name_with_guard};
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

    // Verify initial task status resolution via DB (status only):
    // - root: InProgress (has children)
    // - planning: InProgress (composite, was Ready then auto-transitioned)
    // - execution: Pending (depends on planning)
    // - api-plan: Ready (no dependencies within planning)
    // - api-plan-review: Pending (depends on api-plan)
    use palette_domain::task::{TaskId, TaskStatus};

    let root = state
        .db
        .get_task_state(&TaskId::new("2026/feature-x"))
        .unwrap()
        .unwrap();
    assert_eq!(root.status, TaskStatus::InProgress);

    let planning = state
        .db
        .get_task_state(&TaskId::new("2026/feature-x/planning"))
        .unwrap()
        .unwrap();
    assert_eq!(planning.status, TaskStatus::InProgress);

    let execution = state
        .db
        .get_task_state(&TaskId::new("2026/feature-x/execution"))
        .unwrap()
        .unwrap();
    assert_eq!(execution.status, TaskStatus::Pending);

    let api_plan = state
        .db
        .get_task_state(&TaskId::new("2026/feature-x/planning/api-plan"))
        .unwrap()
        .unwrap();
    assert_eq!(api_plan.status, TaskStatus::Ready);

    let api_plan_review = state
        .db
        .get_task_state(&TaskId::new("2026/feature-x/planning/api-plan-review"))
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
        jobs[0].task_id.to_string(),
        "2026/feature-x/planning/api-plan"
    );
    assert_eq!(jobs[0].job_type, palette_domain::job::JobType::Craft);
    assert_eq!(
        jobs[0].status,
        palette_domain::job::JobStatus::Craft(palette_domain::job::CraftStatus::Todo)
    );
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

    use palette_domain::job::{CraftStatus, JobFilter, JobStatus as JStatus, JobType};
    use palette_domain::task::{TaskId, TaskStatus};

    // step-a should have a Job in Todo state (craft)
    let jobs = state
        .db
        .list_jobs(&JobFilter {
            job_type: None,
            status: None,
            assignee: None,
        })
        .unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].task_id.to_string(), "2026/cascade-test/step-a");

    // step-b should be Pending (depends on step-a)
    let step_b = state
        .db
        .get_task_state(&TaskId::new("2026/cascade-test/step-b"))
        .unwrap()
        .unwrap();
    assert_eq!(step_b.status, TaskStatus::Pending);

    // Simulate Job completion by sending a StatusChanged(Done) effect to the orchestrator.
    let job_id = &jobs[0].id;
    state
        .db
        .update_job_status(job_id, JStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .db
        .update_job_status(job_id, JStatus::Craft(CraftStatus::InReview))
        .unwrap();
    state
        .db
        .update_job_status(job_id, JStatus::Craft(CraftStatus::Done))
        .unwrap();

    // Send StatusChanged effect to trigger orchestrator processing
    use palette_domain::rule::RuleEffect;
    use palette_domain::server::ServerEvent;
    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::StatusChanged {
            job_id: job_id.clone(),
            new_status: JStatus::Craft(CraftStatus::Done),
        }],
    });

    // Give the orchestrator event loop time to process
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Verify: step-a task should be Done
    let step_a = state
        .db
        .get_task_state(&TaskId::new("2026/cascade-test/step-a"))
        .unwrap()
        .unwrap();
    assert_eq!(step_a.status, TaskStatus::Completed);

    // Verify: step-b should now be Ready (dependency satisfied)
    let step_b = state
        .db
        .get_task_state(&TaskId::new("2026/cascade-test/step-b"))
        .unwrap()
        .unwrap();
    assert_eq!(step_b.status, TaskStatus::Ready);

    // Now complete step-b's job to trigger full workflow completion
    let step_b_job = state
        .db
        .create_job(&palette_domain::job::CreateJobRequest {
            id: Some(palette_domain::job::JobId::new("C-step-b")),
            task_id: TaskId::new("2026/cascade-test/step-b"),
            job_type: JobType::Craft,
            title: "step-b".to_string(),
            plan_path: "test/step-b".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repository: None,
        })
        .unwrap();
    state
        .db
        .update_job_status(&step_b_job.id, JStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .db
        .update_job_status(&step_b_job.id, JStatus::Craft(CraftStatus::InReview))
        .unwrap();
    state
        .db
        .update_job_status(&step_b_job.id, JStatus::Craft(CraftStatus::Done))
        .unwrap();

    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::StatusChanged {
            job_id: step_b_job.id.clone(),
            new_status: JStatus::Craft(CraftStatus::Done),
        }],
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // step-b task should be Done
    let step_b = state
        .db
        .get_task_state(&TaskId::new("2026/cascade-test/step-b"))
        .unwrap()
        .unwrap();
    assert_eq!(step_b.status, TaskStatus::Completed);

    // Root task should be Done (all children complete)
    let root = state
        .db
        .get_task_state(&TaskId::new("2026/cascade-test"))
        .unwrap()
        .unwrap();
    assert_eq!(root.status, TaskStatus::Completed);
}

/// Craft InReview should propagate task completion through the task tree,
/// unblocking sibling review tasks and dependent tasks.
#[tokio::test]
async fn craft_in_review_cascades_to_review_task() {
    let (session, _guard) = test_session_name_with_guard("wf-ir-cascade");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();

    // Blueprint: step-a has craft + review; step-b depends on step-a
    let yaml = r#"
task:
  id: e2e/ir-test
  title: InReview cascade test

children:
  - id: step-a
    children:
      - id: craft
        type: craft
        plan_path: test/step-a-craft
      - id: review
        type: review
        depends_on: [craft]
  - id: step-b
    depends_on: [step-a]
    children:
      - id: craft
        type: craft
        plan_path: test/step-b-craft
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

    use palette_domain::job::{CraftStatus, JobFilter, JobStatus as JStatus, ReviewStatus};
    use palette_domain::task::{TaskId, TaskStatus};

    // step-a/craft should have a Job in Todo state
    let jobs = state.db.list_jobs(&JobFilter::default()).unwrap();
    assert_eq!(jobs.len(), 1, "only step-a/craft job should exist");
    let craft_job_id = &jobs[0].id;

    // step-a/review should be Pending
    let review_task = state
        .db
        .get_task_state(&TaskId::new("e2e/ir-test/step-a/review"))
        .unwrap()
        .unwrap();
    assert_eq!(review_task.status, TaskStatus::Pending);

    // Simulate craft job reaching InReview (member stop hook path)
    state
        .db
        .update_job_status(craft_job_id, JStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .db
        .update_job_status(craft_job_id, JStatus::Craft(CraftStatus::InReview))
        .unwrap();

    // Send StatusChanged(InReview) — this is what the stop hook sends
    use palette_domain::rule::RuleEffect;
    use palette_domain::server::ServerEvent;
    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::StatusChanged {
            job_id: craft_job_id.clone(),
            new_status: JStatus::Craft(CraftStatus::InReview),
        }],
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Verify: step-a/craft task should be Done
    let craft_task = state
        .db
        .get_task_state(&TaskId::new("e2e/ir-test/step-a/craft"))
        .unwrap()
        .unwrap();
    assert_eq!(craft_task.status, TaskStatus::Completed);

    // Verify: step-a/review task should now be Ready (dependency on craft satisfied)
    let review_task = state
        .db
        .get_task_state(&TaskId::new("e2e/ir-test/step-a/review"))
        .unwrap()
        .unwrap();
    assert_eq!(review_task.status, TaskStatus::Ready);

    // Verify: a review Job was created for step-a/review
    let all_jobs = state.db.list_jobs(&JobFilter::default()).unwrap();
    let review_jobs: Vec<_> = all_jobs
        .iter()
        .filter(|j| j.task_id.to_string() == "e2e/ir-test/step-a/review")
        .collect();
    assert_eq!(review_jobs.len(), 1, "review job should be created");
    assert_eq!(
        review_jobs[0].status,
        JStatus::Review(ReviewStatus::Todo),
        "review job should be in Todo status for AutoAssign"
    );

    // step-b should still be Pending (step-a composite is not Done yet)
    let step_b = state
        .db
        .get_task_state(&TaskId::new("e2e/ir-test/step-b"))
        .unwrap()
        .unwrap();
    assert_eq!(step_b.status, TaskStatus::Pending);
}

/// Full task tree cascade: craft InReview → review created → review Done →
/// parent composite Done → sibling step-b Ready → step-b craft → workflow complete.
#[tokio::test]
async fn full_task_tree_cascade_with_review() {
    let (session, _guard) = test_session_name_with_guard("wf-full-cascade");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();

    let yaml = r#"
task:
  id: e2e/full
  title: Full cascade test

children:
  - id: step-a
    children:
      - id: craft
        type: craft
        plan_path: test/a-craft
      - id: review
        type: review
        depends_on: [craft]
  - id: step-b
    depends_on: [step-a]
    children:
      - id: craft
        type: craft
        plan_path: test/b-craft
      - id: review
        type: review
        depends_on: [craft]
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
    let resp_body: serde_json::Value = resp.json().await.unwrap();
    let workflow_id =
        palette_domain::workflow::WorkflowId::new(resp_body["workflow_id"].as_str().unwrap());

    use palette_domain::job::{CraftStatus, JobFilter, JobStatus as JStatus, ReviewStatus};
    use palette_domain::rule::RuleEffect;
    use palette_domain::server::ServerEvent;
    use palette_domain::task::{TaskId, TaskStatus};
    use palette_domain::workflow::WorkflowStatus;

    let wait = || tokio::time::sleep(tokio::time::Duration::from_millis(200));

    // --- Phase 1: step-a craft InReview ---
    let jobs = state.db.list_jobs(&JobFilter::default()).unwrap();
    assert_eq!(jobs.len(), 1);
    let craft_a_id = jobs[0].id.clone();

    state
        .db
        .update_job_status(&craft_a_id, JStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .db
        .update_job_status(&craft_a_id, JStatus::Craft(CraftStatus::InReview))
        .unwrap();

    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::StatusChanged {
            job_id: craft_a_id.clone(),
            new_status: JStatus::Craft(CraftStatus::InReview),
        }],
    });
    wait().await;

    // step-a/craft task Done, step-a/review task Ready, review Job created
    assert_eq!(
        state
            .db
            .get_task_state(&TaskId::new("e2e/full/step-a/craft"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Completed
    );
    assert_eq!(
        state
            .db
            .get_task_state(&TaskId::new("e2e/full/step-a/review"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Ready
    );

    let all_jobs = state.db.list_jobs(&JobFilter::default()).unwrap();
    let review_a_job = all_jobs
        .iter()
        .find(|j| j.task_id.as_ref() == "e2e/full/step-a/review")
        .expect("review job should exist");
    let review_a_id = review_a_job.id.clone();

    // step-b still Pending
    assert_eq!(
        state
            .db
            .get_task_state(&TaskId::new("e2e/full/step-b"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Pending
    );

    // --- Phase 2: step-a review Done (approved) ---
    // Review jobs start as Todo; transition to InProgress via assignment
    state
        .db
        .assign_job(
            &review_a_id,
            &palette_domain::agent::AgentId::new("reviewer-a"),
        )
        .unwrap();
    // assign_job sets InProgress

    // Submit approved verdict
    use palette_domain::review::{SubmitReviewRequest, Verdict};
    let sub = state
        .db
        .submit_review(
            &review_a_id,
            &SubmitReviewRequest {
                verdict: Verdict::Approved,
                summary: Some("LGTM".to_string()),
                comments: vec![],
            },
        )
        .unwrap();

    let engine = palette_domain::rule::RuleEngine::new(&*state.db, 5);
    let effects = engine.on_review_submitted(&review_a_id, &sub).unwrap();

    // Effects should include craft_a Done (approved completes both review and craft jobs)
    let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    wait().await;

    // step-a/review task should be Done
    assert_eq!(
        state
            .db
            .get_task_state(&TaskId::new("e2e/full/step-a/review"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Completed
    );

    // step-a composite should be Done (both children Done)
    assert_eq!(
        state
            .db
            .get_task_state(&TaskId::new("e2e/full/step-a"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Completed
    );

    // step-b should now be Ready → InProgress (composite auto-transitions)
    let step_b_status = state
        .db
        .get_task_state(&TaskId::new("e2e/full/step-b"))
        .unwrap()
        .unwrap()
        .status;
    assert_eq!(step_b_status, TaskStatus::InProgress);

    // step-b/craft should be Ready with a Job created
    assert_eq!(
        state
            .db
            .get_task_state(&TaskId::new("e2e/full/step-b/craft"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Ready
    );

    let all_jobs = state.db.list_jobs(&JobFilter::default()).unwrap();
    let craft_b_job = all_jobs
        .iter()
        .find(|j| j.task_id.as_ref() == "e2e/full/step-b/craft")
        .expect("step-b craft job should exist");
    let craft_b_id = craft_b_job.id.clone();

    // --- Phase 3: step-b craft InReview → review → Done → workflow complete ---
    state
        .db
        .update_job_status(&craft_b_id, JStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .db
        .update_job_status(&craft_b_id, JStatus::Craft(CraftStatus::InReview))
        .unwrap();

    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::StatusChanged {
            job_id: craft_b_id.clone(),
            new_status: JStatus::Craft(CraftStatus::InReview),
        }],
    });
    wait().await;

    // step-b/review should be Ready with a Job
    assert_eq!(
        state
            .db
            .get_task_state(&TaskId::new("e2e/full/step-b/review"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Ready
    );

    let all_jobs = state.db.list_jobs(&JobFilter::default()).unwrap();
    let review_b_job = all_jobs
        .iter()
        .find(|j| j.task_id.as_ref() == "e2e/full/step-b/review")
        .expect("step-b review job should exist");
    let review_b_id = review_b_job.id.clone();

    // Approve step-b review
    state
        .db
        .assign_job(
            &review_b_id,
            &palette_domain::agent::AgentId::new("reviewer-b"),
        )
        .unwrap();

    let sub = state
        .db
        .submit_review(
            &review_b_id,
            &SubmitReviewRequest {
                verdict: Verdict::Approved,
                summary: Some("LGTM".to_string()),
                comments: vec![],
            },
        )
        .unwrap();

    let effects = engine.on_review_submitted(&review_b_id, &sub).unwrap();
    let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    wait().await;

    // step-b composite Done
    assert_eq!(
        state
            .db
            .get_task_state(&TaskId::new("e2e/full/step-b"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Completed
    );

    // Root task Done
    assert_eq!(
        state
            .db
            .get_task_state(&TaskId::new("e2e/full"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Completed
    );

    // Workflow completed
    let wf = state.db.get_workflow(&workflow_id).unwrap().unwrap();
    assert_eq!(wf.status, WorkflowStatus::Completed);
}

/// Craft job must NOT be marked Done until ALL sibling review jobs are Done.
#[tokio::test]
async fn craft_waits_for_all_reviews_before_done() {
    let (session, _guard) = test_session_name_with_guard("wf-multi-review");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();

    // Blueprint: craft with two review siblings
    let yaml = r#"
task:
  id: e2e/multi-review
  title: Multi review test

children:
  - id: step
    children:
      - id: craft
        type: craft
        plan_path: test/craft
      - id: review-1
        type: review
        depends_on: [craft]
      - id: review-2
        type: review
        depends_on: [craft]
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

    use palette_domain::job::{CraftStatus, JobFilter, JobStatus as JStatus};
    use palette_domain::rule::RuleEffect;
    use palette_domain::server::ServerEvent;
    let wait = || tokio::time::sleep(tokio::time::Duration::from_millis(200));

    // Craft → InReview
    let jobs = state.db.list_jobs(&JobFilter::default()).unwrap();
    let craft_id = jobs[0].id.clone();
    state
        .db
        .update_job_status(&craft_id, JStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .db
        .update_job_status(&craft_id, JStatus::Craft(CraftStatus::InReview))
        .unwrap();

    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::StatusChanged {
            job_id: craft_id.clone(),
            new_status: JStatus::Craft(CraftStatus::InReview),
        }],
    });
    wait().await;

    // Both review jobs should be created (review-1 and review-2 under step)
    let all_jobs = state.db.list_jobs(&JobFilter::default()).unwrap();
    let mut review_jobs: Vec<_> = all_jobs
        .iter()
        .filter(|j| {
            j.task_id
                .as_ref()
                .starts_with("e2e/multi-review/step/review")
        })
        .collect();
    review_jobs.sort_by_key(|j| j.task_id.to_string());
    assert_eq!(review_jobs.len(), 2, "both review jobs should be created");

    let review_1_id = review_jobs[0].id.clone();
    let review_2_id = review_jobs[1].id.clone();

    // Approve only review-1
    state
        .db
        .assign_job(
            &review_1_id,
            &palette_domain::agent::AgentId::new("reviewer-1"),
        )
        .unwrap();

    use palette_domain::review::{SubmitReviewRequest, Verdict};
    let sub = state
        .db
        .submit_review(
            &review_1_id,
            &SubmitReviewRequest {
                verdict: Verdict::Approved,
                summary: Some("LGTM".to_string()),
                comments: vec![],
            },
        )
        .unwrap();

    let engine = palette_domain::rule::RuleEngine::new(&*state.db, 5);
    let effects = engine.on_review_submitted(&review_1_id, &sub).unwrap();
    let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    wait().await;

    // Craft job should still be InReview (not Done) because review-2 is not yet approved
    let craft_job = state.db.get_job(&craft_id).unwrap().unwrap();
    assert_eq!(
        craft_job.status,
        JStatus::Craft(CraftStatus::InReview),
        "craft job must stay InReview until all reviews are Done"
    );

    // Now approve review-2
    state
        .db
        .assign_job(
            &review_2_id,
            &palette_domain::agent::AgentId::new("reviewer-2"),
        )
        .unwrap();

    let sub = state
        .db
        .submit_review(
            &review_2_id,
            &SubmitReviewRequest {
                verdict: Verdict::Approved,
                summary: Some("LGTM".to_string()),
                comments: vec![],
            },
        )
        .unwrap();

    let effects = engine.on_review_submitted(&review_2_id, &sub).unwrap();
    let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    wait().await;

    // NOW craft job should be Done
    let craft_job = state.db.get_job(&craft_id).unwrap().unwrap();
    assert_eq!(
        craft_job.status,
        JStatus::Craft(CraftStatus::Done),
        "craft job should be Done after all reviews approved"
    );
}
