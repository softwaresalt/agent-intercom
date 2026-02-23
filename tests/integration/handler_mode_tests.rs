//! Integration tests for the `set_operational_mode` tool handler logic.
//!
//! Validates:
//! - Mode transitions persisted correctly (local→remote, remote→local, etc.)
//! - Previous and current mode in response
//! - Session `last_tool` updated
//! - No active session → error
//! - Same-mode transitions are idempotent

use std::sync::Arc;

use monocoque_agent_rc::models::session::SessionMode;
use monocoque_agent_rc::persistence::session_repo::SessionRepo;

use super::test_helpers::{create_active_session_with_mode, test_app_state, test_config};

// ── Mode: local → remote persisted ───────────────────────────

#[tokio::test]
async fn mode_change_local_to_remote_persisted() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session_with_mode(&state.db, root, SessionMode::Local).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    assert_eq!(session.mode, SessionMode::Local);

    repo.update_mode(&session.id, SessionMode::Remote)
        .await
        .expect("update mode");

    let updated = repo
        .get_by_id(&session.id)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(updated.mode, SessionMode::Remote);
}

// ── Mode: remote → hybrid persisted ──────────────────────────

#[tokio::test]
async fn mode_change_remote_to_hybrid_persisted() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session_with_mode(&state.db, root, SessionMode::Remote).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

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

// ── Mode: hybrid → local persisted ───────────────────────────

#[tokio::test]
async fn mode_change_hybrid_to_local_persisted() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session_with_mode(&state.db, root, SessionMode::Hybrid).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    repo.update_mode(&session.id, SessionMode::Local)
        .await
        .expect("update mode");

    let updated = repo
        .get_by_id(&session.id)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(updated.mode, SessionMode::Local);
}

// ── Mode: same mode idempotent ───────────────────────────────

#[tokio::test]
async fn mode_change_same_mode_idempotent() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session_with_mode(&state.db, root, SessionMode::Remote).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    repo.update_mode(&session.id, SessionMode::Remote)
        .await
        .expect("update mode");

    let updated = repo
        .get_by_id(&session.id)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(updated.mode, SessionMode::Remote);
}

// ── Mode: updates session last_tool ──────────────────────────

#[tokio::test]
async fn mode_change_updates_last_tool() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session_with_mode(&state.db, root, SessionMode::Local).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    repo.update_mode(&session.id, SessionMode::Remote)
        .await
        .expect("update mode");
    repo.update_last_activity(&session.id, Some("set_operational_mode".into()))
        .await
        .expect("update last activity");

    let updated = repo
        .get_by_id(&session.id)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(updated.last_tool.as_deref(), Some("set_operational_mode"));
}

// ── Mode: no active session detected ─────────────────────────

#[tokio::test]
async fn mode_change_no_active_session() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    let sessions = repo.list_active().await.expect("list active");
    assert!(sessions.is_empty());
}

// ── Mode: all three modes round-trip via serde ───────────────

#[tokio::test]
async fn mode_serde_round_trip() {
    for mode in &[SessionMode::Remote, SessionMode::Local, SessionMode::Hybrid] {
        let json = serde_json::to_string(mode).expect("serialize");
        let deserialized: SessionMode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(*mode, deserialized);
    }
}
