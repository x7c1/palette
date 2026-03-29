mod helper;

use helper::{capture_pane, jid, spawn_server, test_session_name_with_guard, wid};
use palette_domain::job::{CreateJobRequest, JobStatus, JobType, ReviewStatus};
use palette_domain::task::TaskId;
use palette_domain::terminal::TerminalTarget;
use palette_domain::worker::{ContainerId, WorkerRole, WorkerStatus};
use palette_domain::workflow::WorkflowId;
use palette_tmux::TmuxManager;
use palette_usecase::data_store::{CreateTaskRequest, InsertWorkerRequest};
use serde_json::json;
use std::io::Write;

fn write_blueprint_file(yaml: &str) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(yaml.as_bytes()).unwrap();
    f
}

fn tid(wf_id: &str, key_path: &str) -> TaskId {
    TaskId::new(format!("{wf_id}:{key_path}"))
}

fn insert_worker(
    state: &palette_server::AppState,
    id: &str,
    role: WorkerRole,
    supervisor_id: Option<&str>,
    terminal_target: &TerminalTarget,
    status: WorkerStatus,
    task_id: &str,
    workflow_id: &WorkflowId,
) {
    state
        .interactor
        .data_store
        .insert_worker(&InsertWorkerRequest {
            id: wid(id),
            workflow_id: workflow_id.clone(),
            role,
            status,
            supervisor_id: supervisor_id.map(wid),
            container_id: ContainerId::new("stub"),
            terminal_target: terminal_target.clone(),
            session_id: None,
            task_id: TaskId::new(task_id),
        })
        .unwrap();
}

/// Scenario 3: Multiple review members stop while review integrator is working.
/// Event notifications are queued and delivered one at a time on each leader stop.
#[tokio::test]
async fn scenario3_message_queuing_to_leader() {
    let (session, _guard) = test_session_name_with_guard("scenario3");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let ri_pane = tmux.create_target("review-integrator").unwrap();
    let member_a_pane = tmux.create_target("member-a").unwrap();
    let member_b_pane = tmux.create_target("member-b").unwrap();

    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;

    // Set up workflow and tasks for review jobs
    let wf_id = WorkflowId::new("wf-scenario3");
    state
        .interactor
        .data_store
        .create_workflow(&wf_id, "test/blueprint.yaml")
        .unwrap();
    let task_a = TaskId::new("task-R-A");
    let task_b = TaskId::new("task-R-B");
    let task_ri = TaskId::new("task-ri");
    state
        .interactor
        .data_store
        .create_task(&CreateTaskRequest {
            id: task_ri.clone(),
            workflow_id: wf_id.clone(),
        })
        .unwrap();
    state
        .interactor
        .data_store
        .create_task(&CreateTaskRequest {
            id: task_a.clone(),
            workflow_id: wf_id.clone(),
        })
        .unwrap();
    state
        .interactor
        .data_store
        .create_task(&CreateTaskRequest {
            id: task_b.clone(),
            workflow_id: wf_id.clone(),
        })
        .unwrap();

    // Register workers in DB
    insert_worker(
        &state,
        "review-integrator-1",
        WorkerRole::ReviewIntegrator,
        None,
        &ri_pane,
        WorkerStatus::Working,
        "task-ri",
        &wf_id,
    );
    insert_worker(
        &state,
        "member-a",
        WorkerRole::Member,
        Some("review-integrator-1"),
        &member_a_pane,
        WorkerStatus::Working,
        "task-R-A",
        &wf_id,
    );
    insert_worker(
        &state,
        "member-b",
        WorkerRole::Member,
        Some("review-integrator-1"),
        &member_b_pane,
        WorkerStatus::Working,
        "task-R-B",
        &wf_id,
    );

    let client = reqwest::Client::new();

    // Create review jobs and assign them
    state
        .interactor
        .data_store
        .create_job(&CreateJobRequest {
            task_id: task_a,
            id: Some(jid("R-A")),
            job_type: JobType::Review,
            title: "Review A".to_string(),
            plan_path: "test/R-A".to_string(),
            assignee_id: None,
            priority: None,
            repository: None,
        })
        .unwrap();
    state
        .interactor
        .data_store
        .create_job(&CreateJobRequest {
            task_id: task_b,
            id: Some(jid("R-B")),
            job_type: JobType::Review,
            title: "Review B".to_string(),
            plan_path: "test/R-B".to_string(),
            assignee_id: None,
            priority: None,
            repository: None,
        })
        .unwrap();

    state
        .interactor
        .data_store
        .update_job_status(&jid("R-A"), JobStatus::Review(ReviewStatus::Todo))
        .unwrap();
    state
        .interactor
        .data_store
        .assign_job(&jid("R-A"), &wid("member-a"), JobType::Review)
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&jid("R-B"), JobStatus::Review(ReviewStatus::Todo))
        .unwrap();
    state
        .interactor
        .data_store
        .assign_job(&jid("R-B"), &wid("member-b"), JobType::Review)
        .unwrap();

    // --- Both review members stop while review integrator is Working ---

    // member-a stops
    let resp = client
        .post(format!("{base_url}/hooks/stop?worker_id=member-a"))
        .json(&json!({"last_assistant_message": "review findings A"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // member-b stops
    let resp = client
        .post(format!("{base_url}/hooks/stop?worker_id=member-b"))
        .json(&json!({"last_assistant_message": "review findings B"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Review integrator is Working, so both notifications should be queued
    assert!(
        state
            .interactor
            .data_store
            .has_pending_messages(&wid("review-integrator-1"))
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
            "{base_url}/hooks/stop?worker_id=review-integrator-1"
        ))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let content = capture_pane(&ri_pane);
    assert!(
        content.contains("[review] member=member-a job=R-A type=review_complete"),
        "first stop should deliver member-a review, got: {content}"
    );

    // RI should still have pending messages (member-b event)
    assert!(
        state
            .interactor
            .data_store
            .has_pending_messages(&wid("review-integrator-1"))
            .unwrap(),
        "RI should still have pending message for member-b"
    );

    // --- RI stops (second time) → second queued message delivered ---
    let resp = client
        .post(format!(
            "{base_url}/hooks/stop?worker_id=review-integrator-1"
        ))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let content = capture_pane(&ri_pane);
    assert!(
        content.contains("[review] member=member-b job=R-B type=review_complete"),
        "second stop should deliver member-b review, got: {content}"
    );

    // Queue should now be empty
    assert!(
        !state
            .interactor
            .data_store
            .has_pending_messages(&wid("review-integrator-1"))
            .unwrap(),
        "RI queue should be empty after all deliveries"
    );
}

/// Dynamic supervisor lifecycle:
/// - Workflow start spawns root + phase-a supervisors
/// - phase-a completion destroys its supervisor, phase-b gets a new one
/// - Workflow completion destroys all supervisors
#[tokio::test]
async fn dynamic_supervisor_lifecycle() {
    use palette_domain::job::{CraftStatus, JobFilter, JobStatus as JStatus};
    use palette_domain::rule::RuleEffect;
    use palette_domain::server::ServerEvent;
    use palette_domain::task::TaskStatus;
    use palette_domain::workflow::WorkflowStatus;

    let yaml = r#"
task:
  key: sup-test
  children:
    - key: phase-a
      children:
        - key: craft
          type: craft
          plan_path: test/a-craft
    - key: phase-b
      depends_on: [phase-a]
      children:
        - key: craft
          type: craft
          plan_path: test/b-craft
"#;

    let (session, _guard) = test_session_name_with_guard("dyn-sup");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();
    let blueprint_file = write_blueprint_file(yaml);

    let wait = || tokio::time::sleep(tokio::time::Duration::from_millis(300));

    // --- Phase 1: Start workflow ---
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
    let wf_id = body["workflow_id"].as_str().unwrap();
    let workflow_id = palette_domain::workflow::WorkflowId::new(wf_id);

    wait().await;

    // Verify task states
    assert_eq!(
        state
            .interactor
            .data_store
            .get_task_state(&tid(wf_id, "sup-test"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::InProgress
    );
    assert_eq!(
        state
            .interactor
            .data_store
            .get_task_state(&tid(wf_id, "sup-test/phase-a"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::InProgress
    );
    assert_eq!(
        state
            .interactor
            .data_store
            .get_task_state(&tid(wf_id, "sup-test/phase-b"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Pending
    );

    // Verify supervisors: root + phase-a = 2
    {
        let supervisors = state
            .interactor
            .data_store
            .list_supervisors(&workflow_id)
            .unwrap();
        assert_eq!(
            supervisors.len(),
            2,
            "should have root + phase-a supervisors, got: {:?}",
            supervisors
                .iter()
                .map(|s| (&s.id, &s.task_id))
                .collect::<Vec<_>>()
        );
        assert!(
            state
                .interactor
                .data_store
                .find_supervisor_for_task(&tid(wf_id, "sup-test"))
                .unwrap()
                .is_some(),
            "root supervisor should exist"
        );
        assert!(
            state
                .interactor
                .data_store
                .find_supervisor_for_task(&tid(wf_id, "sup-test/phase-a"))
                .unwrap()
                .is_some(),
            "phase-a supervisor should exist"
        );
    }

    // --- Phase 2: Complete phase-a/craft ---
    let jobs = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    assert_eq!(jobs.len(), 1, "only phase-a/craft job should exist");
    let craft_a_id = jobs[0].id.clone();

    state
        .interactor
        .data_store
        .update_job_status(&craft_a_id, JStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&craft_a_id, JStatus::Craft(CraftStatus::InReview))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&craft_a_id, JStatus::Craft(CraftStatus::Done))
        .unwrap();

    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::JobCompleted {
            job_id: craft_a_id.clone(),
        }],
    });
    wait().await;

    // phase-a should be Completed
    assert_eq!(
        state
            .interactor
            .data_store
            .get_task_state(&tid(wf_id, "sup-test/phase-a"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Completed
    );
    // phase-b should be InProgress (dependency satisfied, pure composite activated)
    assert_eq!(
        state
            .interactor
            .data_store
            .get_task_state(&tid(wf_id, "sup-test/phase-b"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::InProgress
    );

    // Verify supervisors: root + phase-b = 2 (phase-a destroyed, phase-b spawned)
    {
        let supervisors = state
            .interactor
            .data_store
            .list_supervisors(&workflow_id)
            .unwrap();
        assert_eq!(
            supervisors.len(),
            2,
            "should have root + phase-b supervisors, got: {:?}",
            supervisors
                .iter()
                .map(|s| (&s.id, &s.task_id))
                .collect::<Vec<_>>()
        );
        assert!(
            state
                .interactor
                .data_store
                .find_supervisor_for_task(&tid(wf_id, "sup-test/phase-a"))
                .unwrap()
                .is_none(),
            "phase-a supervisor should be destroyed"
        );
        assert!(
            state
                .interactor
                .data_store
                .find_supervisor_for_task(&tid(wf_id, "sup-test/phase-b"))
                .unwrap()
                .is_some(),
            "phase-b supervisor should exist"
        );
    }

    // --- Phase 3: Complete phase-b/craft ---
    let all_jobs = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    let craft_b = all_jobs
        .iter()
        .find(|j| j.task_id.as_ref() == tid(wf_id, "sup-test/phase-b/craft").to_string())
        .expect("phase-b/craft job should exist");
    let craft_b_id = craft_b.id.clone();

    state
        .interactor
        .data_store
        .update_job_status(&craft_b_id, JStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&craft_b_id, JStatus::Craft(CraftStatus::InReview))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&craft_b_id, JStatus::Craft(CraftStatus::Done))
        .unwrap();

    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::JobCompleted {
            job_id: craft_b_id.clone(),
        }],
    });
    wait().await;

    // All supervisors should be destroyed
    {
        let supervisors = state
            .interactor
            .data_store
            .list_supervisors(&workflow_id)
            .unwrap();
        assert_eq!(
            supervisors.len(),
            0,
            "all supervisors should be destroyed, got: {:?}",
            supervisors
                .iter()
                .map(|s| (&s.id, &s.task_id))
                .collect::<Vec<_>>()
        );
    }

    // Workflow should be completed
    let wf = state
        .interactor
        .data_store
        .get_workflow(&workflow_id)
        .unwrap()
        .unwrap();
    assert_eq!(wf.status, WorkflowStatus::Completed);
}

/// Dynamic ReviewIntegrator lifecycle:
/// - Craft InReview spawns ReviewIntegrator for review-integrate composite
/// - All reviews approved → ReviewIntegrator destroyed → workflow complete
#[tokio::test]
async fn dynamic_review_integrator_lifecycle() {
    use palette_domain::job::{CraftStatus, JobFilter, JobStatus as JStatus, JobType};
    use palette_domain::review::{SubmitReviewRequest, Verdict};
    use palette_domain::rule::RuleEffect;
    use palette_domain::server::ServerEvent;
    use palette_domain::workflow::WorkflowStatus;

    let yaml = r#"
task:
  key: ri-test
  children:
    - key: craft
      type: craft
      plan_path: test/craft
      children:
        - key: review-integrate
          type: review
          children:
            - key: review-1
              type: review
            - key: review-2
              type: review
"#;

    let (session, _guard) = test_session_name_with_guard("dyn-ri");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();
    let blueprint_file = write_blueprint_file(yaml);

    // Insert workers needed for assign_job FK constraints
    helper::setup_worker(&*state.interactor.data_store, "reviewer-1");
    helper::setup_worker(&*state.interactor.data_store, "reviewer-2");
    helper::setup_worker(&*state.interactor.data_store, "ri-agent");

    let wait = || tokio::time::sleep(tokio::time::Duration::from_millis(300));

    // Start workflow
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
    let wf_id = body["workflow_id"].as_str().unwrap();
    let workflow_id = palette_domain::workflow::WorkflowId::new(wf_id);

    wait().await;

    // Phase 1: Only root supervisor (craft composite doesn't get one)
    {
        let supervisors = state
            .interactor
            .data_store
            .list_supervisors(&workflow_id)
            .unwrap();
        assert_eq!(
            supervisors.len(),
            1,
            "should have root supervisor only, got: {:?}",
            supervisors
                .iter()
                .map(|s| (&s.id, &s.task_id, &s.role))
                .collect::<Vec<_>>()
        );
    }

    // Phase 2: Craft → InReview → should spawn ReviewIntegrator
    let jobs = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    assert_eq!(jobs.len(), 1);
    let craft_id = jobs[0].id.clone();

    state
        .interactor
        .data_store
        .update_job_status(&craft_id, JStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&craft_id, JStatus::Craft(CraftStatus::InReview))
        .unwrap();

    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::CraftReadyForReview {
            craft_job_id: craft_id.clone(),
        }],
    });
    wait().await;

    // Verify: ReviewIntegrator spawned + review jobs created
    {
        let supervisors = state
            .interactor
            .data_store
            .list_supervisors(&workflow_id)
            .unwrap();
        assert_eq!(
            supervisors.len(),
            2,
            "should have root Leader + ReviewIntegrator, got: {:?}",
            supervisors
                .iter()
                .map(|s| (&s.id, &s.task_id, &s.role))
                .collect::<Vec<_>>()
        );
        let ri_sup = state
            .interactor
            .data_store
            .find_supervisor_for_task(&tid(wf_id, "ri-test/craft/review-integrate"))
            .unwrap()
            .expect("ReviewIntegrator supervisor should exist");
        assert_eq!(ri_sup.role, WorkerRole::ReviewIntegrator);
    }

    // review-1 and review-2 jobs should exist
    let all_jobs = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    let review_jobs: Vec<_> = all_jobs
        .iter()
        .filter(|j| j.job_type == JobType::Review)
        .collect();
    // review-integrate has a Review Job + review-1 + review-2
    assert!(
        review_jobs.len() >= 2,
        "at least review-1 and review-2 jobs should exist, got {} review jobs",
        review_jobs.len()
    );

    // Phase 3: Approve review-1 and review-2
    let review_1_job = all_jobs
        .iter()
        .find(|j| {
            j.task_id.as_ref() == tid(wf_id, "ri-test/craft/review-integrate/review-1").to_string()
        })
        .expect("review-1 job should exist");
    let review_2_job = all_jobs
        .iter()
        .find(|j| {
            j.task_id.as_ref() == tid(wf_id, "ri-test/craft/review-integrate/review-2").to_string()
        })
        .expect("review-2 job should exist");

    // Approve review-1
    state
        .interactor
        .data_store
        .assign_job(&review_1_job.id, &wid("reviewer-1"), JobType::Review)
        .unwrap();
    let sub = state
        .interactor
        .data_store
        .submit_review(
            &review_1_job.id,
            &SubmitReviewRequest {
                verdict: Verdict::Approved,
                summary: Some("LGTM".to_string()),
                comments: vec![],
            },
        )
        .unwrap();
    let effects = palette_usecase::RuleEngine::new(
        state.interactor.data_store.as_ref(),
        state.max_review_rounds,
    )
    .on_review_submitted(&review_1_job.id, &sub)
    .unwrap();
    let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    wait().await;

    // Approve review-2
    state
        .interactor
        .data_store
        .assign_job(&review_2_job.id, &wid("reviewer-2"), JobType::Review)
        .unwrap();
    let sub = state
        .interactor
        .data_store
        .submit_review(
            &review_2_job.id,
            &SubmitReviewRequest {
                verdict: Verdict::Approved,
                summary: Some("LGTM".to_string()),
                comments: vec![],
            },
        )
        .unwrap();
    let effects = palette_usecase::RuleEngine::new(
        state.interactor.data_store.as_ref(),
        state.max_review_rounds,
    )
    .on_review_submitted(&review_2_job.id, &sub)
    .unwrap();
    let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    wait().await;

    // Simulate ReviewIntegrator's integration decision:
    // In production, the RI agent aggregates child reviews and submits a verdict.
    let ri_job = all_jobs
        .iter()
        .find(|j| j.task_id.as_ref() == tid(wf_id, "ri-test/craft/review-integrate").to_string())
        .expect("review-integrate job should exist");
    state
        .interactor
        .data_store
        .assign_job(&ri_job.id, &wid("ri-agent"), JobType::Review)
        .unwrap();
    let sub = state
        .interactor
        .data_store
        .submit_review(
            &ri_job.id,
            &SubmitReviewRequest {
                verdict: Verdict::Approved,
                summary: Some("All reviews approved, integration OK".to_string()),
                comments: vec![],
            },
        )
        .unwrap();
    let effects = palette_usecase::RuleEngine::new(
        state.interactor.data_store.as_ref(),
        state.max_review_rounds,
    )
    .on_review_submitted(&ri_job.id, &sub)
    .unwrap();
    let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    wait().await;

    // All supervisors should be destroyed
    {
        let supervisors = state
            .interactor
            .data_store
            .list_supervisors(&workflow_id)
            .unwrap();
        assert_eq!(
            supervisors.len(),
            0,
            "all supervisors should be destroyed after workflow completion, got: {:?}",
            supervisors
                .iter()
                .map(|s| (&s.id, &s.task_id))
                .collect::<Vec<_>>()
        );
    }

    // Workflow should be completed
    let wf = state
        .interactor
        .data_store
        .get_workflow(&workflow_id)
        .unwrap()
        .unwrap();
    assert_eq!(wf.status, WorkflowStatus::Completed);
}
