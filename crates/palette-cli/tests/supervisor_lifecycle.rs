mod helper;

use helper::{spawn_server, test_session_name_with_guard, tid, wid, write_blueprint_file};
use palette_domain::job::{CraftStatus, JobFilter, JobStatus, JobType};
use palette_domain::review::{SubmitReviewRequest, Verdict};
use palette_domain::server::ServerEvent;
use palette_domain::task::TaskStatus;
use palette_domain::workflow::WorkflowStatus;
use palette_tmux::TmuxManager;

/// Dynamic supervisor lifecycle:
/// - Workflow start spawns root + phase-a supervisors
/// - phase-a completion destroys its supervisor, phase-b gets a new one
/// - Workflow completion destroys all supervisors
#[tokio::test]
async fn dynamic_supervisor_lifecycle() {
    let yaml = r#"
task:
  key: sup-test
  children:
    - key: phase-a
      children:
        - key: craft
          type: craft
          plan_path: test/a-craft
          repository:
            name: x7c1/palette
            branch: main
          children:
            - key: review
              type: review
    - key: phase-b
      depends_on: [phase-a]
      children:
        - key: craft
          type: craft
          plan_path: test/b-craft
          repository:
            name: x7c1/palette
            branch: main
          children:
            - key: review
              type: review
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
    let workflow_id = palette_domain::workflow::WorkflowId::parse(wf_id).unwrap();

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

    // --- Phase 2: Complete phase-a/craft through review ---
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
        .update_job_status(&craft_a_id, JobStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&craft_a_id, JobStatus::Craft(CraftStatus::InReview))
        .unwrap();

    // CraftReadyForReview triggers review job creation
    let _ = state.event_tx.send(ServerEvent::CraftReadyForReview {
        craft_job_id: craft_a_id.clone(),
    });
    wait().await;

    // Approve the review to complete the craft
    let all_jobs_a = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    let review_a = all_jobs_a
        .iter()
        .find(|j| j.task_id.as_ref() == tid(wf_id, "sup-test/phase-a/craft/review").to_string())
        .expect("phase-a/craft/review job should exist");

    helper::setup_worker(&*state.interactor.data_store, "reviewer-a");
    state
        .interactor
        .data_store
        .assign_job(&review_a.id, &wid("reviewer-a"), JobType::Review)
        .unwrap();
    let _sub = state
        .interactor
        .data_store
        .submit_review(
            &review_a.id,
            &SubmitReviewRequest {
                verdict: Verdict::Approved,
                summary: Some("LGTM".to_string()),
                comments: vec![],
            },
        )
        .unwrap();
    let _ = state.event_tx.send(ServerEvent::ReviewSubmitted {
        review_job_id: review_a.id.clone(),
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

    // --- Phase 3: Complete phase-b/craft through review ---
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
        .update_job_status(&craft_b_id, JobStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&craft_b_id, JobStatus::Craft(CraftStatus::InReview))
        .unwrap();

    let _ = state.event_tx.send(ServerEvent::CraftReadyForReview {
        craft_job_id: craft_b_id.clone(),
    });
    wait().await;

    // Approve the review
    let all_jobs_b = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    let review_b = all_jobs_b
        .iter()
        .find(|j| j.task_id.as_ref() == tid(wf_id, "sup-test/phase-b/craft/review").to_string())
        .expect("phase-b/craft/review job should exist");

    helper::setup_worker(&*state.interactor.data_store, "reviewer-b");
    state
        .interactor
        .data_store
        .assign_job(&review_b.id, &wid("reviewer-b"), JobType::Review)
        .unwrap();
    let _sub = state
        .interactor
        .data_store
        .submit_review(
            &review_b.id,
            &SubmitReviewRequest {
                verdict: Verdict::Approved,
                summary: Some("LGTM".to_string()),
                comments: vec![],
            },
        )
        .unwrap();
    let _ = state.event_tx.send(ServerEvent::ReviewSubmitted {
        review_job_id: review_b.id.clone(),
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
