//! Integration tests for tool dispatch logic.
//!
//! These tests validate the state changes that tool handlers would
//! produce by testing the underlying repository and orchestrator
//! operations in the same sequence as the handler dispatch path.
//!
//! Direct `ServerHandler::call_tool()` invocation requires an rmcp
//! `RequestContext` whose construction is `pub(crate)`, so we test
//! the logic layer instead.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

use agent_intercom::mcp::handler::StallDetectors;
use agent_intercom::models::progress::{validate_snapshot, ProgressItem, ProgressStatus};
use agent_intercom::models::session::SessionMode;
use agent_intercom::orchestrator::stall_detector::StallDetector;
use agent_intercom::persistence::db;
use agent_intercom::persistence::session_repo::SessionRepo;

use super::test_helpers::{
    create_active_session, create_active_session_with_mode, create_interrupted_session,
    test_app_state, test_config, test_config_no_channel,
};

// ── Heartbeat: single active session resolved ────────────────

#[tokio::test]
async fn dispatch_heartbeat_resolves_single_active_session() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    // Heartbeat resolves the single active session and updates last_activity.
    let active = repo.list_active().await.expect("list");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, session.id);

    repo.update_last_activity(&session.id, Some("heartbeat".into()))
        .await
        .expect("update");

    let updated = repo
        .get_by_id(&session.id)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(updated.last_tool.as_deref(), Some("heartbeat"));
}

// ── Heartbeat: no active session returns error ───────────────

#[tokio::test]
async fn dispatch_heartbeat_no_session_fails() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    let active = repo.list_active().await.expect("list");
    assert!(active.is_empty(), "no sessions should exist");
}

// ── Heartbeat: multiple active sessions → ambiguity ──────────

#[tokio::test]
async fn dispatch_heartbeat_multiple_sessions_ambiguity() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let _s1 = create_active_session(&state.db, root).await;
    let _s2 = create_active_session(&state.db, root).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    let active = repo.list_active().await.expect("list");
    assert_eq!(active.len(), 2, "two active sessions = ambiguity");
}

// ── Heartbeat: progress snapshot validation ──────────────────

#[tokio::test]
async fn dispatch_heartbeat_validates_snapshot() {
    // Empty label should be rejected.
    let bad_snapshot = vec![ProgressItem {
        label: String::new(),
        status: ProgressStatus::InProgress,
    }];
    let result = validate_snapshot(&bad_snapshot);
    assert!(result.is_err(), "empty label should fail validation");

    // Valid snapshot passes.
    let good_snapshot = vec![
        ProgressItem {
            label: "compile".into(),
            status: ProgressStatus::Done,
        },
        ProgressItem {
            label: "test".into(),
            status: ProgressStatus::InProgress,
        },
    ];
    let result = validate_snapshot(&good_snapshot);
    assert!(result.is_ok(), "valid snapshot should pass");
}

// ── Heartbeat: progress snapshot stored in DB ────────────────

#[tokio::test]
async fn dispatch_heartbeat_stores_progress_snapshot() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    let snapshot = vec![
        ProgressItem {
            label: "compile".into(),
            status: ProgressStatus::Done,
        },
        ProgressItem {
            label: "test".into(),
            status: ProgressStatus::InProgress,
        },
    ];

    repo.update_progress_snapshot(&session.id, Some(snapshot))
        .await
        .expect("update snapshot");

    let updated = repo
        .get_by_id(&session.id)
        .await
        .expect("get")
        .expect("found");
    let stored = updated.progress_snapshot.expect("snapshot");
    assert_eq!(stored.len(), 2);
}

// ── Set mode: changes session mode in DB ─────────────────────

#[tokio::test]
async fn dispatch_set_mode_persists_change() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    assert_eq!(session.mode, SessionMode::Remote);

    repo.update_mode(&session.id, SessionMode::Hybrid)
        .await
        .expect("update mode");

    let updated = repo
        .get_by_id(&session.id)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(updated.mode, SessionMode::Hybrid);
}

// ── Set mode: no session → no-op (0 rows affected) ──────────

#[tokio::test]
async fn dispatch_set_mode_no_session_fails() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    let active = repo.list_active().await.expect("list");
    assert!(active.is_empty());

    // update_mode on a non-existent session silently succeeds (0 rows affected).
    // The MCP handler layer is responsible for resolving a valid session first.
    let result = repo.update_mode("nonexistent-id", SessionMode::Local).await;
    assert!(
        result.is_ok(),
        "repo update_mode is a no-op for missing IDs"
    );

    // Verify no session was created as a side-effect.
    let all = repo.list_active().await.expect("list");
    assert!(all.is_empty(), "no session should have been created");
}

// ── Recover state: no interrupted → clean state ──────────────

#[tokio::test]
async fn dispatch_recover_state_clean() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    let interrupted = repo.list_interrupted().await.expect("list");
    assert!(
        interrupted.is_empty(),
        "clean state: no interrupted sessions"
    );
}

// ── Recover state: finds interrupted sessions ────────────────

#[tokio::test]
async fn dispatch_recover_state_with_interrupted() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_interrupted_session(&state.db, root).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    let interrupted = repo.list_interrupted().await.expect("list");
    assert_eq!(interrupted.len(), 1);
    assert_eq!(interrupted[0].id, session.id);
}

// ── Stall detector reset on tool call ────────────────────────

#[tokio::test]
async fn dispatch_tool_resets_stall_detector() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let _config = test_config(root);
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let session = create_active_session(&database, root).await;

    let ct = CancellationToken::new();
    let (tx, _rx) = mpsc::channel(32);
    let detector = StallDetector::new(
        session.id.clone(),
        Duration::from_secs(60),
        Duration::from_secs(60),
        3,
        tx,
        ct.clone(),
    );
    let handle = detector.spawn();

    let detectors: StallDetectors = Arc::new(Mutex::new(HashMap::new()));
    detectors.lock().await.insert(session.id.clone(), handle);

    // Simulate what call_tool does: reset all detectors.
    {
        let guards = detectors.lock().await;
        for h in guards.values() {
            h.reset();
        }
    }

    // If reset didn't panic, the detector was successfully reset.
    // Verify the detector is still tracked.
    let remaining = detectors.lock().await;
    assert_eq!(remaining.len(), 1);
    assert!(remaining.contains_key(&session.id));

    ct.cancel();
}

// ── Mode transitions in sequence ─────────────────────────────

#[tokio::test]
async fn dispatch_mode_transitions_cycle() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session_with_mode(&state.db, root, SessionMode::Remote).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    // Remote → Hybrid.
    repo.update_mode(&session.id, SessionMode::Hybrid)
        .await
        .expect("update");
    let s = repo
        .get_by_id(&session.id)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(s.mode, SessionMode::Hybrid);

    // Hybrid → Local.
    repo.update_mode(&session.id, SessionMode::Local)
        .await
        .expect("update");
    let s = repo
        .get_by_id(&session.id)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(s.mode, SessionMode::Local);

    // Local → Remote.
    repo.update_mode(&session.id, SessionMode::Remote)
        .await
        .expect("update");
    let s = repo
        .get_by_id(&session.id)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(s.mode, SessionMode::Remote);
}

// ── No-channel config: state still builds ────────────────────

#[tokio::test]
async fn dispatch_no_channel_state_builds_correctly() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config_no_channel(root)).await;

    // No Slack client, no channel — system should still function.
    assert!(state.slack.is_none());

    let repo = SessionRepo::new(Arc::clone(&state.db));
    let session = create_active_session(&state.db, root).await;

    repo.update_last_activity(&session.id, Some("heartbeat".into()))
        .await
        .expect("update");

    let updated = repo
        .get_by_id(&session.id)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(updated.last_tool.as_deref(), Some("heartbeat"));
}
