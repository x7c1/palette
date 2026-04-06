mod helper;

use helper::{CreateJobRequest, CreateTaskRequest, JobDetail, JobStatus, JobType, ReviewStatus};
use helper::{WorkerRole, WorkerStatus, WorkflowId};
use helper::{capture_pane, insert_worker, spawn_server, test_session_name_with_guard, wid};
use palette_domain::task::TaskId;
use palette_tmux::TmuxManager;
use serde_json::json;

/// Multiple review members stop while review integrator is working.
/// Event notifications are queued and delivered one at a time on each supervisor stop.
#[tokio::test]
async fn message_queuing_to_supervisor() {
    let (session, _guard) = test_session_name_with_guard("scenario3");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let ri_pane = tmux.create_target("review-integrator").unwrap();
    let member_a_pane = tmux.create_target("member-a").unwrap();
    let member_b_pane = tmux.create_target("member-b").unwrap();

    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;

    // Set up workflow and tasks for review jobs
    let wf_id = WorkflowId::parse("wf-scenario3").unwrap();
    state
        .interactor
        .data_store
        .create_workflow(&wf_id, "test/blueprint.yaml")
        .unwrap();
    let task_a = TaskId::parse("wf-scenario3:task-R-A").unwrap();
    let task_b = TaskId::parse("wf-scenario3:task-R-B").unwrap();
    let task_ri = TaskId::parse("wf-scenario3:task-ri").unwrap();
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
        "wf-scenario3:task-ri",
        &wf_id,
    );
    insert_worker(
        &state,
        "member-a",
        WorkerRole::Member,
        Some("review-integrator-1"),
        &member_a_pane,
        WorkerStatus::Working,
        "wf-scenario3:task-R-A",
        &wf_id,
    );
    insert_worker(
        &state,
        "member-b",
        WorkerRole::Member,
        Some("review-integrator-1"),
        &member_b_pane,
        WorkerStatus::Working,
        "wf-scenario3:task-R-B",
        &wf_id,
    );

    let client = reqwest::Client::new();

    // Create review jobs and assign them
    let job_a = state
        .interactor
        .data_store
        .create_job(&CreateJobRequest::new(
            task_a,
            palette_domain::job::Title::parse("Review A").unwrap(),
            palette_domain::job::PlanPath::parse("test/R-A").unwrap(),
            None,
            None,
            JobDetail::Review,
        ))
        .unwrap();
    let job_b = state
        .interactor
        .data_store
        .create_job(&CreateJobRequest::new(
            task_b,
            palette_domain::job::Title::parse("Review B").unwrap(),
            palette_domain::job::PlanPath::parse("test/R-B").unwrap(),
            None,
            None,
            JobDetail::Review,
        ))
        .unwrap();

    state
        .interactor
        .data_store
        .update_job_status(&job_a.id, JobStatus::Review(ReviewStatus::Todo))
        .unwrap();
    state
        .interactor
        .data_store
        .assign_job(&job_a.id, &wid("member-a"), JobType::Review)
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&job_b.id, JobStatus::Review(ReviewStatus::Todo))
        .unwrap();
    state
        .interactor
        .data_store
        .assign_job(&job_b.id, &wid("member-b"), JobType::Review)
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
        content.contains(&format!(
            "[review] member=member-a job={} type=review_complete",
            job_a.id
        )),
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
        content.contains(&format!(
            "[review] member=member-b job={} type=review_complete",
            job_b.id
        )),
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
