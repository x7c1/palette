mod helper;

use helper::{CreateJobRequest, CreateTaskRequest, JobDetail, JobStatus, JobType, ReviewStatus};
use helper::{WorkerRole, WorkerStatus, WorkflowId};
use helper::{capture_pane, insert_worker, spawn_server, test_session_name_with_guard, wid};
use palette_domain::task::TaskId;
use palette_tmux::TmuxManager;
use serde_json::json;

/// Consecutive permission prompts from a reviewer are delivered to the
/// ReviewIntegrator one at a time. Each cycle:
///   1. Reviewer sends notification (permission prompt) → message enqueued for RI
///   2. RI is Idle → message delivered immediately → RI becomes Working
///   3. RI stops (approved the prompt) → RI becomes Idle
///   4. Repeat
///
/// Regression test for delivery stalling after the 2nd notification.
#[tokio::test]
async fn sequential_delivery() {
    let (session, _guard) = test_session_name_with_guard("perm-prompt");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let ri_pane = tmux.create_target("review-integrator").unwrap();
    let reviewer_pane = tmux.create_target("reviewer").unwrap();

    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;

    // Set up workflow and tasks
    let wf_id = WorkflowId::parse("wf-perm-prompt").unwrap();
    state
        .interactor
        .data_store
        .create_workflow(&wf_id, "test/blueprint.yaml")
        .unwrap();

    let task_ri = TaskId::parse("wf-perm-prompt:task-ri").unwrap();
    let task_r = TaskId::parse("wf-perm-prompt:task-r").unwrap();
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
            id: task_r.clone(),
            workflow_id: wf_id.clone(),
        })
        .unwrap();

    // Register workers: RI starts Idle (ready to receive), reviewer starts Working
    insert_worker(
        &state,
        "ri-1",
        WorkerRole::ReviewIntegrator,
        None,
        &ri_pane,
        WorkerStatus::Idle,
        "wf-perm-prompt:task-ri",
        &wf_id,
    );
    insert_worker(
        &state,
        "reviewer-1",
        WorkerRole::Member,
        Some("ri-1"),
        &reviewer_pane,
        WorkerStatus::Working,
        "wf-perm-prompt:task-r",
        &wf_id,
    );

    let client = reqwest::Client::new();

    // Create and assign a review job
    let review_job = state
        .interactor
        .data_store
        .create_job(&CreateJobRequest::new(
            task_r,
            palette_domain::job::Title::parse("Review 1").unwrap(),
            Some(palette_domain::job::PlanPath::parse("test/R-1").unwrap()),
            None,
            None,
            JobDetail::Review {
                perspective: None,
                target: palette_domain::job::ReviewTarget::CraftOutput,
            },
        ))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&review_job.id, JobStatus::Review(ReviewStatus::Todo))
        .unwrap();
    state
        .interactor
        .data_store
        .assign_job(&review_job.id, &wid("reviewer-1"), JobType::Review)
        .unwrap();

    let wait = || tokio::time::sleep(std::time::Duration::from_millis(500));

    // Run 4 cycles of: notification → delivery → RI stop
    for round in 1..=4 {
        // Reviewer sends permission prompt notification
        let resp = client
            .post(format!(
                "{base_url}/hooks/notification?worker_id=reviewer-1"
            ))
            .json(&json!({}))
            .send()
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            200,
            "round {round}: notification should succeed"
        );

        wait().await;

        // Verify message was delivered to RI pane
        let content = capture_pane(&ri_pane);
        assert!(
            content.contains("[event] member=reviewer-1 type=permission_prompt"),
            "round {round}: notification should appear in RI pane, got: {content}"
        );

        // RI should be Working (delivery transitions Idle → Working)
        let ri = state
            .interactor
            .data_store
            .find_worker(&wid("ri-1"))
            .unwrap()
            .unwrap();
        assert_eq!(
            ri.status,
            WorkerStatus::Working,
            "round {round}: RI should be Working after delivery"
        );

        // Queue should be empty (message was delivered)
        assert!(
            !state
                .interactor
                .data_store
                .has_pending_messages(&wid("ri-1"))
                .unwrap(),
            "round {round}: RI queue should be empty after delivery"
        );

        // RI stops (approved the permission prompt) → Idle
        let resp = client
            .post(format!("{base_url}/hooks/stop?worker_id=ri-1"))
            .json(&json!({}))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "round {round}: RI stop should succeed");

        wait().await;

        // RI should be Idle, ready for next notification
        let ri = state
            .interactor
            .data_store
            .find_worker(&wid("ri-1"))
            .unwrap()
            .unwrap();
        assert_eq!(
            ri.status,
            WorkerStatus::Idle,
            "round {round}: RI should be Idle after stop"
        );
    }
}

/// Permission prompts that arrive while the ReviewIntegrator is Working
/// are queued and delivered one at a time on each RI stop.
///
/// Flow:
///   1. RI is Working
///   2. Two notifications arrive → both queued
///   3. RI stops → first message delivered
///   4. RI stops → second message delivered
///   5. Third notification arrives → RI is Idle → delivered immediately
#[tokio::test]
async fn queued_while_working() {
    let (session, _guard) = test_session_name_with_guard("perm-queued");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let ri_pane = tmux.create_target("review-integrator").unwrap();
    let reviewer_pane = tmux.create_target("reviewer").unwrap();

    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;

    let wf_id = WorkflowId::parse("wf-perm-queued").unwrap();
    state
        .interactor
        .data_store
        .create_workflow(&wf_id, "test/blueprint.yaml")
        .unwrap();

    let task_ri = TaskId::parse("wf-perm-queued:task-ri").unwrap();
    let task_r = TaskId::parse("wf-perm-queued:task-r").unwrap();
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
            id: task_r.clone(),
            workflow_id: wf_id.clone(),
        })
        .unwrap();

    // RI starts Working (already processing something)
    insert_worker(
        &state,
        "ri-1",
        WorkerRole::ReviewIntegrator,
        None,
        &ri_pane,
        WorkerStatus::Working,
        "wf-perm-queued:task-ri",
        &wf_id,
    );
    insert_worker(
        &state,
        "reviewer-1",
        WorkerRole::Member,
        Some("ri-1"),
        &reviewer_pane,
        WorkerStatus::Working,
        "wf-perm-queued:task-r",
        &wf_id,
    );

    let client = reqwest::Client::new();

    let review_job = state
        .interactor
        .data_store
        .create_job(&CreateJobRequest::new(
            task_r,
            palette_domain::job::Title::parse("Review 1").unwrap(),
            Some(palette_domain::job::PlanPath::parse("test/R-1").unwrap()),
            None,
            None,
            JobDetail::Review {
                perspective: None,
                target: palette_domain::job::ReviewTarget::CraftOutput,
            },
        ))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&review_job.id, JobStatus::Review(ReviewStatus::Todo))
        .unwrap();
    state
        .interactor
        .data_store
        .assign_job(&review_job.id, &wid("reviewer-1"), JobType::Review)
        .unwrap();

    let wait = || tokio::time::sleep(std::time::Duration::from_millis(500));

    // --- Two notifications arrive while RI is Working ---
    for i in 1..=2 {
        let resp = client
            .post(format!(
                "{base_url}/hooks/notification?worker_id=reviewer-1"
            ))
            .json(&json!({}))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "notification {i} should succeed");
    }

    wait().await;

    // Both messages should be queued (RI is Working, can't deliver)
    assert!(
        state
            .interactor
            .data_store
            .has_pending_messages(&wid("ri-1"))
            .unwrap(),
        "RI should have pending messages while Working"
    );

    // RI pane should NOT contain any notification yet
    let content = capture_pane(&ri_pane);
    assert!(
        !content.contains("[event] member=reviewer-1 type=permission_prompt"),
        "RI pane should not have notifications while Working, got: {content}"
    );

    // --- RI stops (first time) → first queued message delivered ---
    let resp = client
        .post(format!("{base_url}/hooks/stop?worker_id=ri-1"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    wait().await;

    let content = capture_pane(&ri_pane);
    assert!(
        content.contains("[event] member=reviewer-1 type=permission_prompt"),
        "first RI stop should deliver a message, got: {content}"
    );

    // Second message should still be queued
    assert!(
        state
            .interactor
            .data_store
            .has_pending_messages(&wid("ri-1"))
            .unwrap(),
        "RI should still have pending message after first stop"
    );

    // --- RI stops (second time) → second queued message delivered ---
    let resp = client
        .post(format!("{base_url}/hooks/stop?worker_id=ri-1"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    wait().await;

    let content = capture_pane(&ri_pane);
    assert!(
        content.contains("[event] member=reviewer-1 type=permission_prompt"),
        "second RI stop should deliver a message, got: {content}"
    );

    // Queue should be empty
    assert!(
        !state
            .interactor
            .data_store
            .has_pending_messages(&wid("ri-1"))
            .unwrap(),
        "RI queue should be empty after both deliveries"
    );

    // RI is Working (delivering msg2 transitioned it from Idle → Working).
    // One more stop to go back to Idle.
    let resp = client
        .post(format!("{base_url}/hooks/stop?worker_id=ri-1"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    wait().await;

    let ri = state
        .interactor
        .data_store
        .find_worker(&wid("ri-1"))
        .unwrap()
        .unwrap();
    assert_eq!(
        ri.status,
        WorkerStatus::Idle,
        "RI should be Idle after final stop"
    );

    // --- Third notification arrives while RI is Idle → delivered immediately ---
    let resp = client
        .post(format!(
            "{base_url}/hooks/notification?worker_id=reviewer-1"
        ))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    wait().await;

    let content = capture_pane(&ri_pane);
    assert!(
        content.contains("[event] member=reviewer-1 type=permission_prompt"),
        "third notification should be delivered immediately, got: {content}"
    );

    assert!(
        !state
            .interactor
            .data_store
            .has_pending_messages(&wid("ri-1"))
            .unwrap(),
        "RI queue should be empty after immediate delivery"
    );
}

/// Concurrent notification + stop: fire both hooks simultaneously to
/// probe for race conditions between enqueue and delivery.
///
/// Each round:
///   1. Ensure RI is Working (so the stop hook will transition to Idle)
///   2. Fire notification (enqueue) and RI stop (Idle transition) at the same time
///   3. Wait, then verify the message was eventually delivered or is still queued
///   4. Drain: keep stopping RI until the queue is empty
///
/// Repeats 10 times to increase the chance of hitting a problematic interleaving.
#[tokio::test]
async fn concurrent_race() {
    let (session, _guard) = test_session_name_with_guard("perm-race");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let ri_pane = tmux.create_target("review-integrator").unwrap();
    let reviewer_pane = tmux.create_target("reviewer").unwrap();

    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;

    let wf_id = WorkflowId::parse("wf-perm-race").unwrap();
    state
        .interactor
        .data_store
        .create_workflow(&wf_id, "test/blueprint.yaml")
        .unwrap();

    let task_ri = TaskId::parse("wf-perm-race:task-ri").unwrap();
    let task_r = TaskId::parse("wf-perm-race:task-r").unwrap();
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
            id: task_r.clone(),
            workflow_id: wf_id.clone(),
        })
        .unwrap();

    insert_worker(
        &state,
        "ri-1",
        WorkerRole::ReviewIntegrator,
        None,
        &ri_pane,
        WorkerStatus::Working,
        "wf-perm-race:task-ri",
        &wf_id,
    );
    insert_worker(
        &state,
        "reviewer-1",
        WorkerRole::Member,
        Some("ri-1"),
        &reviewer_pane,
        WorkerStatus::Working,
        "wf-perm-race:task-r",
        &wf_id,
    );

    let client = reqwest::Client::new();

    let review_job = state
        .interactor
        .data_store
        .create_job(&CreateJobRequest::new(
            task_r,
            palette_domain::job::Title::parse("Review 1").unwrap(),
            Some(palette_domain::job::PlanPath::parse("test/R-1").unwrap()),
            None,
            None,
            JobDetail::Review {
                perspective: None,
                target: palette_domain::job::ReviewTarget::CraftOutput,
            },
        ))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&review_job.id, JobStatus::Review(ReviewStatus::Todo))
        .unwrap();
    state
        .interactor
        .data_store
        .assign_job(&review_job.id, &wid("reviewer-1"), JobType::Review)
        .unwrap();

    let wait = || tokio::time::sleep(std::time::Duration::from_millis(500));

    for round in 1..=10 {
        // Ensure RI is Working before each round so the stop hook will
        // transition it to Idle (if it's already Idle, force it to Working).
        let ri = state
            .interactor
            .data_store
            .find_worker(&wid("ri-1"))
            .unwrap()
            .unwrap();
        if ri.status != WorkerStatus::Working {
            state
                .interactor
                .data_store
                .update_worker_status(&wid("ri-1"), WorkerStatus::Working)
                .unwrap();
        }

        // Fire notification and stop concurrently
        let notification_fut = client
            .post(format!(
                "{base_url}/hooks/notification?worker_id=reviewer-1"
            ))
            .json(&json!({}))
            .send();

        let stop_fut = client
            .post(format!("{base_url}/hooks/stop?worker_id=ri-1"))
            .json(&json!({}))
            .send();

        let (notif_resp, stop_resp) = tokio::join!(notification_fut, stop_fut);
        assert_eq!(notif_resp.unwrap().status(), 200, "round {round}");
        assert_eq!(stop_resp.unwrap().status(), 200, "round {round}");

        wait().await;

        // Drain: stop RI until the queue is empty (at most 3 attempts).
        // The message may have been delivered immediately (RI went Idle
        // before or after enqueue) or may still be queued.
        for attempt in 0..3 {
            if !state
                .interactor
                .data_store
                .has_pending_messages(&wid("ri-1"))
                .unwrap()
            {
                break;
            }
            // RI might be Working from delivery; stop it so next message can be delivered
            let ri = state
                .interactor
                .data_store
                .find_worker(&wid("ri-1"))
                .unwrap()
                .unwrap();
            if ri.status == WorkerStatus::Working {
                let resp = client
                    .post(format!("{base_url}/hooks/stop?worker_id=ri-1"))
                    .json(&json!({}))
                    .send()
                    .await
                    .unwrap();
                assert_eq!(resp.status(), 200);
                wait().await;
            }

            assert!(
                attempt < 2,
                "round {round}: message stuck in queue after {attempt} drain attempts"
            );
        }

        // After draining, the message must have been delivered
        assert!(
            !state
                .interactor
                .data_store
                .has_pending_messages(&wid("ri-1"))
                .unwrap(),
            "round {round}: queue should be empty after drain"
        );

        let content = capture_pane(&ri_pane);
        assert!(
            content.contains("[event] member=reviewer-1 type=permission_prompt"),
            "round {round}: notification should have been delivered, pane: {content}"
        );
    }
}
