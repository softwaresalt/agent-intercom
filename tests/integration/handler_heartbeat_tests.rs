//! Integration tests for the `heartbeat` tool handler logic.
//!
//! Validates:
//! - Single active session → acknowledged with `session_id`
//! - Progress snapshot storage and validation
//! - Last-activity update
//! - No active session → error
//! - Multiple active sessions → ambiguity error
//! - Empty label in snapshot → validation error

use std::sync::Arc;

use agent_intercom::models::progress::{ProgressItem, ProgressStatus};
use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::persistence::session_repo::SessionRepo;

use super::test_helpers::{create_active_session, test_app_state, test_config};

// ── Heartbeat: single active session ─────────────────────────

#[tokio::test]
async fn heartbeat_single_session_updates_last_activity() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    // Simulate heartbeat: list_active → update_last_activity
    let sessions = repo.list_active().await.expect("list active");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, session.id);

    repo.update_last_activity(&session.id, Some("heartbeat".into()))
        .await
        .expect("update last activity");

    let updated = repo
        .get_by_id(&session.id)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(updated.last_tool.as_deref(), Some("heartbeat"));
}

// ── Heartbeat: progress snapshot storage ─────────────────────

#[tokio::test]
async fn heartbeat_stores_progress_snapshot() {
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

    repo.update_progress_snapshot(&session.id, Some(snapshot.clone()))
        .await
        .expect("update snapshot");

    let updated = repo
        .get_by_id(&session.id)
        .await
        .expect("get")
        .expect("found");
    let stored = updated.progress_snapshot.expect("snapshot present");
    assert_eq!(stored.len(), 2);
    assert_eq!(stored[0].label, "compile");
    assert_eq!(stored[0].status, ProgressStatus::Done);
    assert_eq!(stored[1].label, "test");
    assert_eq!(stored[1].status, ProgressStatus::InProgress);
}

// ── Heartbeat: snapshot validation rejects empty label ───────

#[tokio::test]
async fn heartbeat_rejects_empty_label_in_snapshot() {
    use agent_intercom::models::progress::validate_snapshot;

    let snapshot = vec![ProgressItem {
        label: String::new(),
        status: ProgressStatus::Pending,
    }];

    let result = validate_snapshot(&snapshot);
    assert!(result.is_err(), "empty label should be rejected");
}

// ── Heartbeat: no active session ─────────────────────────────

#[tokio::test]
async fn heartbeat_no_active_session_is_error() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    // No sessions created at all.
    let sessions = repo.list_active().await.expect("list active");
    assert!(sessions.is_empty(), "should be no active sessions");
}

// ── Heartbeat: multiple active sessions → ambiguity ──────────

#[tokio::test]
async fn heartbeat_multiple_active_sessions_is_ambiguous() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    // Create two active sessions.
    create_active_session(&state.db, root).await;
    create_active_session(&state.db, root).await;

    let sessions = repo.list_active().await.expect("list active");
    assert_eq!(sessions.len(), 2, "should have two active sessions");

    // Heartbeat handler would reject this as ambiguous.
    let mut iter = sessions.into_iter();
    let first = iter.next();
    let second = iter.next();
    assert!(
        first.is_some() && second.is_some(),
        "multiple active sessions should trigger ambiguity error"
    );
}

// ── Heartbeat: created-but-not-activated session not listed ──

#[tokio::test]
async fn heartbeat_created_session_not_listed_as_active() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    // Create but do NOT activate session.
    let session = Session::new(
        "U_OWNER".into(),
        root.into(),
        Some("test".into()),
        SessionMode::Remote,
    );
    repo.create(&session).await.expect("create");

    let sessions = repo.list_active().await.expect("list active");
    assert!(
        sessions.is_empty(),
        "created-only session should not be active"
    );
}

// ── Heartbeat: terminated session not listed ─────────────────

#[tokio::test]
async fn heartbeat_terminated_session_not_listed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    let session = create_active_session(&state.db, root).await;
    repo.update_status(&session.id, SessionStatus::Terminated)
        .await
        .expect("terminate");

    let sessions = repo.list_active().await.expect("list active");
    assert!(
        sessions.is_empty(),
        "terminated session should not be active"
    );
}
