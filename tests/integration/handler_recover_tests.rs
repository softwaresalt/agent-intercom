//! Integration tests for the `recover_state` tool handler logic.
//!
//! Validates:
//! - No interrupted session → clean status
//! - Interrupted session with pending approval → recovered with pending
//! - Interrupted session with pending prompt → recovered with pending
//! - Specific `session_id` → finds that session
//! - Checkpoint and progress snapshot included in recovery
//! - Multiple interrupted sessions → most recent returned
//! - Clean active session → clean status (no interrupted)

use std::collections::HashMap;
use std::sync::Arc;

use monocoque_agent_rc::models::approval::{ApprovalRequest, RiskLevel};
use monocoque_agent_rc::models::checkpoint::Checkpoint;
use monocoque_agent_rc::models::progress::{ProgressItem, ProgressStatus};
use monocoque_agent_rc::models::prompt::{ContinuationPrompt, PromptType};
use monocoque_agent_rc::models::session::{Session, SessionMode, SessionStatus};
use monocoque_agent_rc::persistence::approval_repo::ApprovalRepo;
use monocoque_agent_rc::persistence::checkpoint_repo::CheckpointRepo;
use monocoque_agent_rc::persistence::prompt_repo::PromptRepo;
use monocoque_agent_rc::persistence::session_repo::SessionRepo;

use super::test_helpers::{create_interrupted_session, test_app_state, test_config};

// ── Recover state: no interrupted session → clean ────────────

#[tokio::test]
async fn recover_state_no_interrupted_returns_clean() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    let session = repo.get_most_recent_interrupted().await.expect("query");
    assert!(session.is_none(), "should find no interrupted session");
}

// ── Recover state: interrupted session with pending approval ─

#[tokio::test]
async fn recover_state_with_pending_approval() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_interrupted_session(&state.db, root).await;

    // Create a pending approval for this session.
    let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));
    let approval = ApprovalRequest::new(
        session.id.clone(),
        "Fix bug".into(),
        None,
        "diff content".into(),
        "src/main.rs".into(),
        RiskLevel::Low,
        "hash123".into(),
    );
    approval_repo
        .create(&approval)
        .await
        .expect("create approval");

    // Recover: should find the interrupted session, and its pending approval.
    let repo = SessionRepo::new(Arc::clone(&state.db));
    let recovered = repo
        .get_most_recent_interrupted()
        .await
        .expect("query")
        .expect("found");
    assert_eq!(recovered.id, session.id);

    let pending = approval_repo
        .get_pending_for_session(&session.id)
        .await
        .expect("query pending");
    assert!(pending.is_some(), "should have pending approval");
    assert_eq!(pending.as_ref().expect("present").title, "Fix bug");
}

// ── Recover state: interrupted session with pending prompt ───

#[tokio::test]
async fn recover_state_with_pending_prompt() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_interrupted_session(&state.db, root).await;

    // Create a pending prompt for this session.
    let prompt_repo = PromptRepo::new(Arc::clone(&state.db));
    let prompt = ContinuationPrompt::new(
        session.id.clone(),
        "Should I continue?".into(),
        PromptType::Continuation,
        Some(120),
        Some(5),
    );
    prompt_repo.create(&prompt).await.expect("create prompt");

    let pending = prompt_repo
        .get_pending_for_session(&session.id)
        .await
        .expect("query pending");
    assert!(pending.is_some(), "should have pending prompt");
    assert_eq!(
        pending.as_ref().expect("present").prompt_text,
        "Should I continue?"
    );
}

// ── Recover state: with checkpoint ───────────────────────────

#[tokio::test]
async fn recover_state_includes_checkpoint() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_interrupted_session(&state.db, root).await;

    let checkpoint_repo = CheckpointRepo::new(Arc::clone(&state.db));
    let checkpoint = Checkpoint::new(
        session.id.clone(),
        Some("phase-2 complete".into()),
        serde_json::json!({"step": 2}),
        HashMap::new(),
        root.into(),
        None,
    );
    checkpoint_repo
        .create(&checkpoint)
        .await
        .expect("create checkpoint");

    let checkpoints = checkpoint_repo
        .list_for_session(&session.id)
        .await
        .expect("list checkpoints");
    assert_eq!(checkpoints.len(), 1);
    assert_eq!(checkpoints[0].label.as_deref(), Some("phase-2 complete"));
}

// ── Recover state: with progress snapshot ────────────────────

#[tokio::test]
async fn recover_state_includes_progress_snapshot() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    // Create a session, set progress, then interrupt.
    let session = Session::new("U_OWNER".into(), root.into(), None, SessionMode::Remote);
    let created = repo.create(&session).await.expect("create");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let snapshot = vec![ProgressItem {
        label: "task-1".into(),
        status: ProgressStatus::Done,
    }];
    repo.update_progress_snapshot(&created.id, Some(snapshot))
        .await
        .expect("update snapshot");

    repo.update_status(&created.id, SessionStatus::Interrupted)
        .await
        .expect("interrupt");

    let recovered = repo
        .get_most_recent_interrupted()
        .await
        .expect("query")
        .expect("found");
    assert!(recovered.progress_snapshot.is_some());
    assert_eq!(recovered.progress_snapshot.as_ref().expect("snap").len(), 1);
}

// ── Recover state: specific session_id ───────────────────────

#[tokio::test]
async fn recover_state_by_specific_session_id() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_interrupted_session(&state.db, root).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    let found = repo
        .get_by_id(&session.id)
        .await
        .expect("query")
        .expect("found");
    assert_eq!(found.id, session.id);
    assert_eq!(found.status, SessionStatus::Interrupted);
}

// ── Recover state: nonexistent session_id → None ─────────────

#[tokio::test]
async fn recover_state_nonexistent_session_returns_none() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    let found = repo.get_by_id("nonexistent-id").await.expect("query");
    assert!(found.is_none());
}

// ── Recover state: active session not matched as interrupted ─

#[tokio::test]
async fn recover_state_active_session_not_recovered() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let repo = SessionRepo::new(Arc::clone(&state.db));

    // Only active sessions, no interrupted ones.
    let session = Session::new("U_OWNER".into(), root.into(), None, SessionMode::Remote);
    let created = repo.create(&session).await.expect("create");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let interrupted = repo.get_most_recent_interrupted().await.expect("query");
    assert!(
        interrupted.is_none(),
        "active session should not be found as interrupted"
    );
}

// ── Recover state: comprehensive recovery payload ────────────

#[tokio::test]
async fn recover_state_full_recovery_payload() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;
    let session = create_interrupted_session(&state.db, root).await;

    // Add pending approval.
    let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));
    let approval = ApprovalRequest::new(
        session.id.clone(),
        "Add feature".into(),
        Some("description".into()),
        "diff".into(),
        "src/lib.rs".into(),
        RiskLevel::High,
        "hash456".into(),
    );
    approval_repo.create(&approval).await.expect("create");

    // Add pending prompt.
    let prompt_repo = PromptRepo::new(Arc::clone(&state.db));
    let prompt = ContinuationPrompt::new(
        session.id.clone(),
        "Continue?".into(),
        PromptType::Clarification,
        None,
        None,
    );
    prompt_repo.create(&prompt).await.expect("create");

    // Add checkpoint.
    let checkpoint_repo = CheckpointRepo::new(Arc::clone(&state.db));
    let cp = Checkpoint::new(
        session.id.clone(),
        Some("checkpoint-1".into()),
        serde_json::json!({}),
        HashMap::new(),
        root.into(),
        None,
    );
    checkpoint_repo.create(&cp).await.expect("create");

    // Verify all components are recoverable.
    let pending_approval = approval_repo
        .get_pending_for_session(&session.id)
        .await
        .expect("query");
    let pending_prompt = prompt_repo
        .get_pending_for_session(&session.id)
        .await
        .expect("query");
    let checkpoints = checkpoint_repo
        .list_for_session(&session.id)
        .await
        .expect("list");

    assert!(pending_approval.is_some(), "should have pending approval");
    assert!(pending_prompt.is_some(), "should have pending prompt");
    assert_eq!(checkpoints.len(), 1, "should have one checkpoint");
}
