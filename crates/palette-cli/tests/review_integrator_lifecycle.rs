mod helper;

use helper::{spawn_server, test_session_name_with_guard, tid, wid, write_blueprint_file};
use palette_domain::job::{CraftStatus, JobFilter, JobStatus, JobType};
use palette_domain::review::{SubmitReviewRequest, Verdict};
use palette_domain::server::ServerEvent;
use palette_domain::worker::WorkerRole;
use palette_domain::workflow::WorkflowStatus;
use palette_tmux::TmuxManager;

/// Dynamic ReviewIntegrator lifecycle:
/// - Craft InReview spawns Approver for review-integrate composite
/// - All reviews approved → ReviewIntegrator spawned → integrates → workflow complete
#[tokio::test]
async fn dynamic_review_integrator_lifecycle() {
    let yaml = r#"
task:
  key: ri-test
  children:
    - key: craft
      type: craft
      plan_path: test/craft
      children:
        - key: review-integrate
          type: review_integrate
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
    let workflow_id = palette_domain::workflow::WorkflowId::parse(wf_id).unwrap();

    wait().await;

    // Phase 1: Only root Approver (craft composite doesn't get one)
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

    // Phase 2: Craft → InReview → should spawn Approver for review composite
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
        .update_job_status(&craft_id, JobStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&craft_id, JobStatus::Craft(CraftStatus::InReview))
        .unwrap();

    let _ = state.event_tx.send(ServerEvent::CraftReadyForReview {
        craft_job_id: craft_id.clone(),
    });
    wait().await;

    // Verify: Approver spawned for review composite + review jobs created
    {
        let supervisors = state
            .interactor
            .data_store
            .list_supervisors(&workflow_id)
            .unwrap();
        assert_eq!(
            supervisors.len(),
            2,
            "should have root approver + review approver, got: {:?}",
            supervisors
                .iter()
                .map(|s| (&s.id, &s.task_id, &s.role))
                .collect::<Vec<_>>()
        );
        let review_sup = state
            .interactor
            .data_store
            .find_supervisor_for_task(&tid(wf_id, "ri-test/craft/review-integrate"))
            .unwrap()
            .expect("review composite supervisor should exist");
        assert_eq!(review_sup.role, WorkerRole::Approver);
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
    let _sub = state
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
    let _ = state.event_tx.send(ServerEvent::ReviewSubmitted {
        review_job_id: review_1_job.id.clone(),
    });
    wait().await;

    // Approve review-2
    state
        .interactor
        .data_store
        .assign_job(&review_2_job.id, &wid("reviewer-2"), JobType::Review)
        .unwrap();
    let _sub = state
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
    let _ = state.event_tx.send(ServerEvent::ReviewSubmitted {
        review_job_id: review_2_job.id.clone(),
    });
    wait().await;

    // Phase 4: All reviewers done → ReviewIntegrator should be spawned
    {
        let supervisors = state
            .interactor
            .data_store
            .list_supervisors(&workflow_id)
            .unwrap();
        let ri_sups: Vec<_> = supervisors
            .iter()
            .filter(|s| s.role == WorkerRole::ReviewIntegrator)
            .collect();
        assert_eq!(
            ri_sups.len(),
            1,
            "ReviewIntegrator should be spawned after all reviewers complete, got supervisors: {:?}",
            supervisors
                .iter()
                .map(|s| (&s.id, &s.task_id, &s.role))
                .collect::<Vec<_>>()
        );
    }

    // Phase 5: Simulate ReviewIntegrator's integration decision
    let ri_job = all_jobs
        .iter()
        .find(|j| j.task_id.as_ref() == tid(wf_id, "ri-test/craft/review-integrate").to_string())
        .expect("review-integrate job should exist");
    state
        .interactor
        .data_store
        .assign_job(&ri_job.id, &wid("ri-agent"), JobType::ReviewIntegrate)
        .unwrap();
    let _sub = state
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
    let _ = state.event_tx.send(ServerEvent::ReviewSubmitted {
        review_job_id: ri_job.id.clone(),
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
