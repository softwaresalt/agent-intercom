//! Integration tests for the retention purge task (T042).
//!
//! Validates:
//! - Expired terminated sessions and all child records are deleted
//! - Active and recent sessions remain untouched
//! - Cascading deletion removes children before parent sessions

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{Duration, Utc};

use monocoque_agent_rc::models::approval::{ApprovalRequest, RiskLevel};
use monocoque_agent_rc::models::checkpoint::Checkpoint;
use monocoque_agent_rc::models::prompt::{ContinuationPrompt, PromptType};
use monocoque_agent_rc::models::session::{Session, SessionMode, SessionStatus};
use monocoque_agent_rc::models::stall::StallAlert;
use monocoque_agent_rc::persistence::{
    approval_repo::ApprovalRepo, checkpoint_repo::CheckpointRepo, db, prompt_repo::PromptRepo,
    retention, session_repo::SessionRepo, stall_repo::StallAlertRepo,
};

/// Create a session that was terminated `days_ago` days in the past.
async fn create_expired_session(
    session_repo: &SessionRepo,
    session_id: &str,
    days_ago: i64,
) -> Session {
    let mut session = Session::new(
        "U_RETENTION".to_owned(),
        "/tmp/retention-test".to_owned(),
        Some("retention test prompt".to_owned()),
        SessionMode::Remote,
    );
    // Override the generated ID for deterministic testing.
    session_id.clone_into(&mut session.id);
    session.status = SessionStatus::Terminated;
    session.terminated_at = Some(Utc::now() - Duration::days(days_ago));
    session_repo.create(&session).await.expect("create session");
    session
}

/// Create a session that is still active (not terminated).
async fn create_active_session(session_repo: &SessionRepo, session_id: &str) -> Session {
    let mut session = Session::new(
        "U_RETENTION".to_owned(),
        "/tmp/retention-test".to_owned(),
        Some("active session prompt".to_owned()),
        SessionMode::Remote,
    );
    session_id.clone_into(&mut session.id);
    session_repo.create(&session).await.expect("create session");
    session
}

/// Create child records (approval, checkpoint, prompt, stall alert) for a session.
async fn create_children(
    session_id: &str,
    approval_repo: &ApprovalRepo,
    checkpoint_repo: &CheckpointRepo,
    prompt_repo: &PromptRepo,
    stall_repo: &StallAlertRepo,
) {
    let approval = ApprovalRequest::new(
        session_id.to_owned(),
        "test approval".to_owned(),
        None,
        "--- diff ---".to_owned(),
        "src/lib.rs".to_owned(),
        RiskLevel::Low,
        "hash123".to_owned(),
    );
    approval_repo
        .create(&approval)
        .await
        .expect("create approval");

    let checkpoint = Checkpoint::new(
        session_id.to_owned(),
        Some("test-checkpoint".to_owned()),
        serde_json::json!({"status": "active"}),
        HashMap::new(),
        "/tmp/workspace".to_owned(),
        None,
    );
    checkpoint_repo
        .create(&checkpoint)
        .await
        .expect("create checkpoint");

    let prompt = ContinuationPrompt::new(
        session_id.to_owned(),
        "Continue?".to_owned(),
        PromptType::Continuation,
        Some(60),
        Some(1),
    );
    prompt_repo.create(&prompt).await.expect("create prompt");

    let alert = StallAlert::new(
        session_id.to_owned(),
        Some("heartbeat".to_owned()),
        Utc::now(),
        120,
        None,
    );
    stall_repo.create(&alert).await.expect("create stall alert");
}

#[tokio::test]
async fn purge_deletes_expired_sessions_and_children() {
    let db = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&db));
    let approval_repo = ApprovalRepo::new(Arc::clone(&db));
    let checkpoint_repo = CheckpointRepo::new(Arc::clone(&db));
    let prompt_repo = PromptRepo::new(Arc::clone(&db));
    let stall_repo = StallAlertRepo::new(Arc::clone(&db));

    // Create an expired session (terminated 45 days ago, retention = 30 days).
    let expired = create_expired_session(&session_repo, "sess-expired", 45).await;
    create_children(
        &expired.id,
        &approval_repo,
        &checkpoint_repo,
        &prompt_repo,
        &stall_repo,
    )
    .await;

    // Create a recent session (terminated 10 days ago — within retention window).
    let recent = create_expired_session(&session_repo, "sess-recent", 10).await;
    create_children(
        &recent.id,
        &approval_repo,
        &checkpoint_repo,
        &prompt_repo,
        &stall_repo,
    )
    .await;

    // Create an active session (not terminated).
    let active = create_active_session(&session_repo, "sess-active").await;
    create_children(
        &active.id,
        &approval_repo,
        &checkpoint_repo,
        &prompt_repo,
        &stall_repo,
    )
    .await;

    // Run purge with 30-day retention.
    retention::purge(&db, 30).await.expect("purge");

    // Expired session and all its children should be gone.
    assert!(
        session_repo
            .get_by_id(&expired.id)
            .await
            .expect("query")
            .is_none(),
        "expired session should be deleted"
    );

    // Recent session should still exist.
    assert!(
        session_repo
            .get_by_id(&recent.id)
            .await
            .expect("query")
            .is_some(),
        "recent session should remain"
    );

    // Active session should still exist.
    assert!(
        session_repo
            .get_by_id(&active.id)
            .await
            .expect("query")
            .is_some(),
        "active session should remain"
    );

    // Recent session's children should be intact.
    let recent_checkpoints = checkpoint_repo
        .list_for_session(&recent.id)
        .await
        .expect("list");
    assert_eq!(
        recent_checkpoints.len(),
        1,
        "recent session children should remain"
    );

    // Active session's children should be intact.
    let active_checkpoints = checkpoint_repo
        .list_for_session(&active.id)
        .await
        .expect("list");
    assert_eq!(
        active_checkpoints.len(),
        1,
        "active session children should remain"
    );

    // Expired session's children should be gone.
    let expired_checkpoints = checkpoint_repo
        .list_for_session(&expired.id)
        .await
        .expect("list");
    assert!(
        expired_checkpoints.is_empty(),
        "expired session children should be deleted"
    );
}

#[tokio::test]
async fn purge_with_no_expired_sessions_is_noop() {
    let db = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&db));

    let active = create_active_session(&session_repo, "sess-noop").await;

    retention::purge(&db, 30).await.expect("purge");

    assert!(
        session_repo
            .get_by_id(&active.id)
            .await
            .expect("query")
            .is_some(),
        "active session should remain untouched"
    );
}

#[tokio::test]
async fn purge_respects_retention_days_config() {
    let db = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&db));

    // Terminated 20 days ago.
    create_expired_session(&session_repo, "sess-border-1", 20).await;

    // With 15-day retention: should be deleted.
    retention::purge(&db, 15).await.expect("purge");
    assert!(
        session_repo
            .get_by_id("sess-border-1")
            .await
            .expect("query")
            .is_none(),
        "session older than retention should be deleted"
    );

    // Create another, terminated 5 days ago.
    create_expired_session(&session_repo, "sess-border-2", 5).await;

    // With 15-day retention: should remain.
    retention::purge(&db, 15).await.expect("purge");
    assert!(
        session_repo
            .get_by_id("sess-border-2")
            .await
            .expect("query")
            .is_some(),
        "session within retention window should remain"
    );
}
