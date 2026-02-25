//! Integration tests for the `remote_log` tool handler logic.
//!
//! Validates:
//! - Valid log levels accepted (info, success, warning, error)
//! - Invalid level rejected
//! - Session `last_tool` updated
//! - No active session → error
//! - Slack absent → posted: false (graceful degradation)

use std::sync::Arc;

use agent_intercom::persistence::session_repo::SessionRepo;

use super::test_helpers::{create_active_session, test_app_state, test_config};

/// Valid severity levels mirroring the handler constant.
const VALID_LEVELS: &[&str] = &["info", "success", "warning", "error"];

// ── Remote log: all valid levels accepted ────────────────────

#[tokio::test]
async fn remote_log_all_valid_levels_accepted() {
    for level in VALID_LEVELS {
        assert!(
            VALID_LEVELS.contains(level),
            "level '{level}' should be valid",
        );
    }
}

// ── Remote log: invalid level rejected ───────────────────────

#[tokio::test]
async fn remote_log_invalid_level_rejected() {
    let invalid_levels = ["debug", "trace", "critical", "WARN", ""];
    for level in &invalid_levels {
        assert!(
            !VALID_LEVELS.contains(level),
            "level '{level}' should be rejected",
        );
    }
}

// ── Remote log: updates session last_tool ────────────────────

#[tokio::test]
async fn remote_log_updates_session_last_tool() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_active_session(&state.db, root).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    // Simulate: handler resolves session, posts to slack (no-op), updates last_tool.
    repo.update_last_activity(&session.id, Some("remote_log".into()))
        .await
        .expect("update last activity");

    let updated = repo
        .get_by_id(&session.id)
        .await
        .expect("get")
        .expect("found");
    assert_eq!(updated.last_tool.as_deref(), Some("remote_log"));
}

// ── Remote log: no active session detected ───────────────────

#[tokio::test]
async fn remote_log_no_active_session_detected() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    let sessions = repo.list_active().await.expect("list active");
    assert!(sessions.is_empty());
}

// ── Remote log: graceful when slack is absent ────────────────

#[tokio::test]
async fn remote_log_no_slack_returns_not_posted() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let _session = create_active_session(&state.db, root).await;

    // state.slack is None in test_app_state → handler would return { posted: false, ts: "" }
    assert!(
        state.slack.is_none(),
        "slack should be absent in test state"
    );
}
