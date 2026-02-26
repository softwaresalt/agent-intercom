//! Cross-cutting edge case tests for MCP tool handler logic.
//!
//! Validates scenarios that span multiple tools or test unusual
//! conditions not covered by individual tool-specific tests.

use std::collections::HashMap;
use std::sync::Arc;

use agent_intercom::mcp::handler::{
    AppState, ApprovalResponse, PromptResponse, StallDetectors, WaitResponse,
};
use agent_intercom::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};
use agent_intercom::models::progress::{ProgressItem, ProgressStatus};
use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::orchestrator::stall_detector::StallDetector;
use agent_intercom::persistence::approval_repo::ApprovalRepo;
use agent_intercom::persistence::db;
use agent_intercom::persistence::session_repo::SessionRepo;
use tokio::sync::Mutex;

use super::test_helpers::{
    create_active_session, test_app_state, test_config, test_config_no_channel,
};

// ═══════════════════════════════════════════════════════════════
//  Stall detector integration
// ═══════════════════════════════════════════════════════════════

// ── Stall detectors: None is handled gracefully ──────────────

#[tokio::test]
async fn stall_detectors_none_no_panic() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;

    // State has stall_detectors: None — handler resets should be no-ops.
    assert!(
        state.stall_detectors.is_none(),
        "default test state should have no stall detectors"
    );
}

// ── Stall detectors: reset on active detector ────────────────

#[tokio::test]
async fn stall_detector_reset_works() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let config = test_config(root);
    let database = Arc::new(db::connect_memory().await.expect("db connect"));

    // Create state with stall detectors enabled.
    let detectors: StallDetectors = Arc::new(Mutex::new(HashMap::new()));

    let state = Arc::new(AppState {
        config: Arc::new(config),
        db: database,
        slack: None,
        pending_approvals: Arc::new(Mutex::new(HashMap::new())),
        pending_prompts: Arc::new(Mutex::new(HashMap::new())),
        pending_waits: Arc::new(Mutex::new(HashMap::new())),
        pending_modal_contexts: Arc::default(),
        stall_detectors: Some(Arc::clone(&detectors)),
        ipc_auth_token: None,
        policy_cache: Arc::default(),
        audit_logger: None,
    });

    let session = create_active_session(&state.db, root).await;

    // Create a stall detector via the builder (StallDetector::new → spawn).
    let cancel = tokio_util::sync::CancellationToken::new();
    let (event_tx, _event_rx) =
        tokio::sync::mpsc::channel::<agent_intercom::orchestrator::stall_detector::StallEvent>(10);
    let detector = StallDetector::new(
        session.id.clone(),
        std::time::Duration::from_secs(300), // inactivity_threshold
        std::time::Duration::from_secs(120), // escalation_interval
        3,                                   // max_retries
        event_tx,
        cancel.clone(),
    );
    let handle = detector.spawn();

    {
        let mut guards = detectors.lock().await;
        guards.insert(session.id.clone(), handle);
    }

    // handler.call_tool resets all detectors before and after every tool call.
    if let Some(ref dets) = state.stall_detectors {
        let guards = dets.lock().await;
        for h in guards.values() {
            h.reset(); // Should not panic.
        }
    }

    // Cancel the stall detector to clean up the background task.
    cancel.cancel();
}

// ═══════════════════════════════════════════════════════════════
//  No-Slack-channel edge cases
// ═══════════════════════════════════════════════════════════════

// ── No channel: effective_channel_id → None ──────────────────

#[tokio::test]
async fn no_channel_effective_channel_is_none() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let config = test_config_no_channel(root);
    let state = test_app_state(config).await;
    // Use the helper-produced config which has empty [slack] channel.

    let server = agent_intercom::mcp::handler::IntercomServer::new(Arc::clone(&state));
    assert_eq!(server.effective_channel_id(), None);
}

// ═══════════════════════════════════════════════════════════════
//  Concurrent operations
// ═══════════════════════════════════════════════════════════════

// ── Concurrent: two pending approvals for different sessions ─

#[tokio::test]
async fn concurrent_pending_approvals_independent() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;

    let (tx1, rx1) = tokio::sync::oneshot::channel::<ApprovalResponse>();
    let (tx2, rx2) = tokio::sync::oneshot::channel::<ApprovalResponse>();

    {
        let mut pending = state.pending_approvals.lock().await;
        pending.insert("req-1".into(), tx1);
        pending.insert("req-2".into(), tx2);
        assert_eq!(pending.len(), 2);
    }

    // Resolve req-1.
    {
        let mut pending = state.pending_approvals.lock().await;
        if let Some(sender) = pending.remove("req-1") {
            let _ = sender.send(ApprovalResponse {
                status: "approved".into(),
                reason: None,
            });
        }
    }

    let resp1 = rx1.await.expect("receive");
    assert_eq!(resp1.status, "approved");

    // req-2 is still pending.
    {
        let pending = state.pending_approvals.lock().await;
        assert!(pending.contains_key("req-2"));
    }

    // Resolve req-2.
    {
        let mut pending = state.pending_approvals.lock().await;
        if let Some(sender) = pending.remove("req-2") {
            let _ = sender.send(ApprovalResponse {
                status: "rejected".into(),
                reason: Some("No".into()),
            });
        }
    }

    let resp2 = rx2.await.expect("receive");
    assert_eq!(resp2.status, "rejected");
}

// ── Concurrent: pending prompts for different sessions ───────

#[tokio::test]
async fn concurrent_pending_prompts_independent() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;

    let (tx1, rx1) = tokio::sync::oneshot::channel::<PromptResponse>();
    let (tx2, rx2) = tokio::sync::oneshot::channel::<PromptResponse>();

    {
        let mut pending = state.pending_prompts.lock().await;
        pending.insert("prompt-a".into(), tx1);
        pending.insert("prompt-b".into(), tx2);
    }

    // Resolve prompt-b first (out of order).
    {
        let mut pending = state.pending_prompts.lock().await;
        if let Some(sender) = pending.remove("prompt-b") {
            let _ = sender.send(PromptResponse {
                decision: "stop".into(),
                instruction: None,
            });
        }
    }

    let resp_b = rx2.await.expect("receive");
    assert_eq!(resp_b.decision, "stop");

    // prompt-a still pending.
    {
        let pending = state.pending_prompts.lock().await;
        assert!(pending.contains_key("prompt-a"));
    }

    drop(rx1); // Cleanup.
}

// ── Concurrent: pending waits for different sessions ─────────

#[tokio::test]
async fn concurrent_pending_waits_independent() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;

    let (tx1, rx1) = tokio::sync::oneshot::channel::<WaitResponse>();
    let (tx2, _rx2) = tokio::sync::oneshot::channel::<WaitResponse>();

    {
        let mut pending = state.pending_waits.lock().await;
        pending.insert("session-x".into(), tx1);
        pending.insert("session-y".into(), tx2);
        assert_eq!(pending.len(), 2);
    }

    // Resolve session-x.
    {
        let mut pending = state.pending_waits.lock().await;
        if let Some(sender) = pending.remove("session-x") {
            let _ = sender.send(WaitResponse {
                status: "resumed".into(),
                instruction: Some("Do X".into()),
            });
        }
    }

    let resp = rx1.await.expect("receive");
    assert_eq!(resp.status, "resumed");
    assert_eq!(resp.instruction.as_deref(), Some("Do X"));
}

// ═══════════════════════════════════════════════════════════════
//  Session state transition edge cases
// ═══════════════════════════════════════════════════════════════

// ── Invalid transitions rejected ─────────────────────────────

#[tokio::test]
async fn invalid_session_transition_rejected() {
    let session = Session::new("U_OWNER".into(), "/test".into(), None, SessionMode::Remote);

    // Created → Paused is invalid (must go Created → Active first).
    assert!(
        !session.can_transition_to(SessionStatus::Paused),
        "Created → Paused should be invalid"
    );
    // Created → Terminated is invalid.
    assert!(
        !session.can_transition_to(SessionStatus::Terminated),
        "Created → Terminated should be invalid"
    );
    // Created → Interrupted is invalid.
    assert!(
        !session.can_transition_to(SessionStatus::Interrupted),
        "Created → Interrupted should be invalid"
    );
}

// ── Valid transitions accepted ───────────────────────────────

#[tokio::test]
async fn valid_session_transitions_accepted() {
    let mut session = Session::new("U_OWNER".into(), "/test".into(), None, SessionMode::Remote);

    // Created → Active.
    assert!(session.can_transition_to(SessionStatus::Active));

    session.status = SessionStatus::Active;
    // Active → Paused.
    assert!(session.can_transition_to(SessionStatus::Paused));
    // Active → Terminated.
    assert!(session.can_transition_to(SessionStatus::Terminated));
    // Active → Interrupted.
    assert!(session.can_transition_to(SessionStatus::Interrupted));

    session.status = SessionStatus::Paused;
    // Paused → Active (resume).
    assert!(session.can_transition_to(SessionStatus::Active));
    // Paused → Terminated.
    assert!(session.can_transition_to(SessionStatus::Terminated));
}

// ═══════════════════════════════════════════════════════════════
//  IPC auth token edge cases
// ═══════════════════════════════════════════════════════════════

// ── IPC auth: None means auth disabled ───────────────────────

#[tokio::test]
async fn ipc_auth_none_means_disabled() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;

    assert!(
        state.ipc_auth_token.is_none(),
        "default test state should have no IPC auth token"
    );
}

// ── IPC auth: Some means enabled ─────────────────────────────

#[tokio::test]
async fn ipc_auth_some_means_enabled() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let config = test_config(root);
    let database = Arc::new(db::connect_memory().await.expect("db connect"));

    let state = Arc::new(AppState {
        config: Arc::new(config),
        db: database,
        slack: None,
        pending_approvals: Arc::new(Mutex::new(HashMap::new())),
        pending_prompts: Arc::new(Mutex::new(HashMap::new())),
        pending_waits: Arc::new(Mutex::new(HashMap::new())),
        pending_modal_contexts: Arc::default(),
        stall_detectors: None,
        ipc_auth_token: Some("test-secret-token".into()),
        policy_cache: Arc::default(),
        audit_logger: None,
    });

    assert_eq!(state.ipc_auth_token.as_deref(), Some("test-secret-token"));
}

// ═══════════════════════════════════════════════════════════════
//  Progress snapshot edge cases
// ═══════════════════════════════════════════════════════════════

// ── Snapshot: clearing snapshot ──────────────────────────────

#[tokio::test]
async fn progress_snapshot_can_be_cleared() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    // Set a snapshot.
    let snapshot = vec![ProgressItem {
        label: "step 1".into(),
        status: ProgressStatus::Done,
    }];
    repo.update_progress_snapshot(&session.id, Some(snapshot))
        .await
        .expect("set snapshot");

    // Clear it.
    repo.update_progress_snapshot(&session.id, None)
        .await
        .expect("clear snapshot");

    let updated = repo
        .get_by_id(&session.id)
        .await
        .expect("get")
        .expect("found");
    assert!(
        updated.progress_snapshot.is_none(),
        "snapshot should be cleared"
    );
}

// ── Snapshot: replacing snapshot completely ───────────────────

#[tokio::test]
async fn progress_snapshot_replace_completely() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    let snap1 = vec![ProgressItem {
        label: "a".into(),
        status: ProgressStatus::Pending,
    }];
    repo.update_progress_snapshot(&session.id, Some(snap1))
        .await
        .expect("set snap1");

    let snap2 = vec![
        ProgressItem {
            label: "x".into(),
            status: ProgressStatus::Done,
        },
        ProgressItem {
            label: "y".into(),
            status: ProgressStatus::InProgress,
        },
    ];
    repo.update_progress_snapshot(&session.id, Some(snap2))
        .await
        .expect("set snap2");

    let updated = repo
        .get_by_id(&session.id)
        .await
        .expect("get")
        .expect("found");
    let snap = updated.progress_snapshot.expect("present");
    assert_eq!(snap.len(), 2);
    assert_eq!(snap[0].label, "x");
    assert_eq!(snap[1].label, "y");
}

// ═══════════════════════════════════════════════════════════════
//  Approval status transition completeness
// ═══════════════════════════════════════════════════════════════

// ── Approval: all status transitions ─────────────────────────

#[tokio::test]
async fn approval_status_transition_completeness() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let repo = ApprovalRepo::new(Arc::clone(&state.db));

    // Test: Pending → Approved → Consumed.
    let approval = ApprovalRequest::new(
        session.id.clone(),
        "Flow test".into(),
        None,
        "diff".into(),
        "file.rs".into(),
        RiskLevel::Low,
        "hash".into(),
    );
    let id = approval.id.clone();
    repo.create(&approval).await.expect("create");

    let pending = repo.get_by_id(&id).await.expect("get").expect("found");
    assert_eq!(pending.status, ApprovalStatus::Pending);

    repo.update_status(&id, ApprovalStatus::Approved)
        .await
        .expect("approve");
    let approved = repo.get_by_id(&id).await.expect("get").expect("found");
    assert_eq!(approved.status, ApprovalStatus::Approved);

    repo.mark_consumed(&id).await.expect("consume");
    let consumed = repo.get_by_id(&id).await.expect("get").expect("found");
    assert_eq!(consumed.status, ApprovalStatus::Consumed);
    assert!(consumed.consumed_at.is_some());

    // Test: Pending → Rejected.
    let approval2 = ApprovalRequest::new(
        session.id.clone(),
        "Reject flow".into(),
        None,
        "diff".into(),
        "file2.rs".into(),
        RiskLevel::High,
        "hash2".into(),
    );
    let id2 = approval2.id.clone();
    repo.create(&approval2).await.expect("create");
    repo.update_status(&id2, ApprovalStatus::Rejected)
        .await
        .expect("reject");
    let rejected = repo.get_by_id(&id2).await.expect("get").expect("found");
    assert_eq!(rejected.status, ApprovalStatus::Rejected);

    // Test: Pending → Expired.
    let approval3 = ApprovalRequest::new(
        session.id.clone(),
        "Expire flow".into(),
        None,
        "diff".into(),
        "file3.rs".into(),
        RiskLevel::Critical,
        "hash3".into(),
    );
    let id3 = approval3.id.clone();
    repo.create(&approval3).await.expect("create");
    repo.update_status(&id3, ApprovalStatus::Expired)
        .await
        .expect("expire");
    let expired = repo.get_by_id(&id3).await.expect("get").expect("found");
    assert_eq!(expired.status, ApprovalStatus::Expired);
}
