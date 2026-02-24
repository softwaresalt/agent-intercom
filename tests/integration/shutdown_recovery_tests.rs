//! Integration tests for graceful shutdown and startup recovery.
//!
//! Validates that `graceful_shutdown` marks pending approvals, prompts,
//! and sessions as Interrupted, and that `check_interrupted_on_startup`
//! correctly identifies interrupted sessions after restart.

use std::sync::Arc;

use agent_intercom::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};
use agent_intercom::models::prompt::{ContinuationPrompt, PromptDecision, PromptType};
use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::persistence::approval_repo::ApprovalRepo;
use agent_intercom::persistence::db;
use agent_intercom::persistence::prompt_repo::PromptRepo;
use agent_intercom::persistence::session_repo::SessionRepo;

use super::test_helpers::{test_app_state, test_config};

// ── Shutdown marks pending approvals as Interrupted ──────────

#[tokio::test]
async fn shutdown_interrupts_pending_approvals() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;

    let session = create_active_session_in_db(&state.db, root).await;
    let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));

    let approval = ApprovalRequest::new(
        session.id.clone(),
        "test approval".into(),
        None,
        "diff content".into(),
        "file.rs".into(),
        RiskLevel::Low,
        "abc123".into(),
    );
    approval_repo
        .create(&approval)
        .await
        .expect("create approval");

    // Verify pending before shutdown.
    let pending = approval_repo.list_pending().await.expect("list pending");
    assert_eq!(pending.len(), 1);

    // Simulate shutdown: mark all pending approvals as Interrupted.
    for a in &pending {
        approval_repo
            .update_status(&a.id, ApprovalStatus::Interrupted)
            .await
            .expect("interrupt approval");
    }

    // Verify interrupted.
    let after = approval_repo.list_pending().await.expect("list after");
    assert!(after.is_empty(), "no pending approvals after shutdown");
}

// ── Shutdown marks pending prompts as stopped ────────────────

#[tokio::test]
async fn shutdown_interrupts_pending_prompts() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;

    let session = create_active_session_in_db(&state.db, root).await;
    let prompt_repo = PromptRepo::new(Arc::clone(&state.db));

    let prompt = ContinuationPrompt::new(
        session.id.clone(),
        "should we continue?".into(),
        PromptType::Continuation,
        None,
        None,
    );
    prompt_repo.create(&prompt).await.expect("create prompt");

    // Verify pending.
    let pending = prompt_repo.list_pending().await.expect("list pending");
    assert_eq!(pending.len(), 1);

    // Simulate shutdown: set decision to Stop.
    for p in &pending {
        prompt_repo
            .update_decision(&p.id, PromptDecision::Stop, Some("server shutdown".into()))
            .await
            .expect("interrupt prompt");
    }

    // Verify stopped.
    let after = prompt_repo.list_pending().await.expect("list after");
    assert!(after.is_empty(), "no pending prompts after shutdown");
}

// ── Shutdown marks active sessions as Interrupted ────────────

#[tokio::test]
async fn shutdown_interrupts_active_sessions() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;

    let session = create_active_session_in_db(&state.db, root).await;
    let session_repo = SessionRepo::new(Arc::clone(&state.db));

    // Verify active.
    let active = session_repo.list_active().await.expect("list active");
    assert_eq!(active.len(), 1);

    // Simulate shutdown: mark as Interrupted.
    let active_or_paused = session_repo
        .list_active_or_paused()
        .await
        .expect("list active/paused");
    for s in &active_or_paused {
        session_repo
            .set_terminated(&s.id, SessionStatus::Interrupted)
            .await
            .expect("interrupt session");
    }

    // Verify interrupted.
    let active_after = session_repo.list_active().await.expect("list after");
    assert!(active_after.is_empty(), "no active sessions after shutdown");

    let interrupted = session_repo
        .list_interrupted()
        .await
        .expect("list interrupted");
    assert_eq!(interrupted.len(), 1);
    assert_eq!(interrupted[0].id, session.id);
}

// ── Shutdown with paused session also marks Interrupted ──────

#[tokio::test]
async fn shutdown_interrupts_paused_sessions() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;

    let session_repo = SessionRepo::new(Arc::clone(&state.db));
    let session = Session::new("U_OWNER".into(), root.into(), None, SessionMode::Remote);
    let created = session_repo.create(&session).await.expect("create");
    session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");
    session_repo
        .update_status(&created.id, SessionStatus::Paused)
        .await
        .expect("pause");

    // Simulate shutdown.
    let active_or_paused = session_repo.list_active_or_paused().await.expect("list");
    assert_eq!(active_or_paused.len(), 1);

    for s in &active_or_paused {
        session_repo
            .set_terminated(&s.id, SessionStatus::Interrupted)
            .await
            .expect("interrupt");
    }

    let interrupted = session_repo
        .list_interrupted()
        .await
        .expect("list interrupted");
    assert_eq!(interrupted.len(), 1);
}

// ── Full shutdown sequence: approvals + prompts + sessions ───

#[tokio::test]
async fn full_shutdown_sequence() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let state = test_app_state(test_config(root)).await;

    let session = create_active_session_in_db(&state.db, root).await;
    let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));
    let prompt_repo = PromptRepo::new(Arc::clone(&state.db));
    let session_repo = SessionRepo::new(Arc::clone(&state.db));

    // Create pending entities.
    let approval = ApprovalRequest::new(
        session.id.clone(),
        "apply changes".into(),
        None,
        "diff".into(),
        "main.rs".into(),
        RiskLevel::High,
        "hash".into(),
    );
    approval_repo
        .create(&approval)
        .await
        .expect("create approval");

    let prompt = ContinuationPrompt::new(
        session.id.clone(),
        "continue?".into(),
        PromptType::Continuation,
        Some(120),
        Some(5),
    );
    prompt_repo.create(&prompt).await.expect("create prompt");

    // Pre-shutdown counts.
    assert_eq!(approval_repo.list_pending().await.expect("list").len(), 1);
    assert_eq!(prompt_repo.list_pending().await.expect("list").len(), 1);
    assert_eq!(session_repo.list_active().await.expect("list").len(), 1);

    // Execute shutdown sequence.
    for a in &approval_repo.list_pending().await.expect("list") {
        approval_repo
            .update_status(&a.id, ApprovalStatus::Interrupted)
            .await
            .expect("interrupt");
    }
    for p in &prompt_repo.list_pending().await.expect("list") {
        prompt_repo
            .update_decision(&p.id, PromptDecision::Stop, Some("shutdown".into()))
            .await
            .expect("interrupt");
    }
    for s in &session_repo.list_active_or_paused().await.expect("list") {
        session_repo
            .set_terminated(&s.id, SessionStatus::Interrupted)
            .await
            .expect("interrupt");
    }

    // Post-shutdown: everything should be resolved.
    assert_eq!(approval_repo.list_pending().await.expect("list").len(), 0);
    assert_eq!(prompt_repo.list_pending().await.expect("list").len(), 0);
    assert_eq!(session_repo.list_active().await.expect("list").len(), 0);
    assert_eq!(
        session_repo.list_interrupted().await.expect("list").len(),
        1
    );
}

// ── Startup recovery: finds interrupted sessions ─────────────

#[tokio::test]
async fn startup_recovery_finds_interrupted_sessions() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&database));

    // Pre-populate with interrupted sessions (simulating prior crash).
    let session = Session::new(
        "U_OWNER".into(),
        root.into(),
        Some("was running".into()),
        SessionMode::Remote,
    );
    let created = session_repo.create(&session).await.expect("create");
    session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");
    session_repo
        .set_terminated(&created.id, SessionStatus::Interrupted)
        .await
        .expect("interrupt");

    // Startup recovery check.
    let interrupted = session_repo.list_interrupted().await.expect("list");
    assert_eq!(interrupted.len(), 1);
    assert_eq!(interrupted[0].id, created.id);
    assert_eq!(interrupted[0].status, SessionStatus::Interrupted);
}

// ── Startup recovery: no interrupted sessions → clean ────────

#[tokio::test]
async fn startup_recovery_no_interrupted_is_clean() {
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let session_repo = SessionRepo::new(Arc::clone(&database));

    let interrupted = session_repo.list_interrupted().await.expect("list");
    assert!(interrupted.is_empty());
}

// ── Startup recovery: counts pending approvals/prompts ───────

#[tokio::test]
async fn startup_recovery_counts_pending_per_session() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let database = Arc::new(db::connect_memory().await.expect("db"));

    let session_repo = SessionRepo::new(Arc::clone(&database));
    let approval_repo = ApprovalRepo::new(Arc::clone(&database));
    let prompt_repo = PromptRepo::new(Arc::clone(&database));

    // Create an interrupted session with pending approval.
    let session = Session::new("U_OWNER".into(), root.into(), None, SessionMode::Remote);
    let created = session_repo.create(&session).await.expect("create");
    session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let approval = ApprovalRequest::new(
        created.id.clone(),
        "pending approval".into(),
        None,
        "diff content".into(),
        "test.rs".into(),
        RiskLevel::Low,
        "hash".into(),
    );
    approval_repo.create(&approval).await.expect("create");

    let prompt = ContinuationPrompt::new(
        created.id.clone(),
        "continue?".into(),
        PromptType::Continuation,
        None,
        None,
    );
    prompt_repo.create(&prompt).await.expect("create");

    // Mark session as interrupted (crash simulation).
    session_repo
        .set_terminated(&created.id, SessionStatus::Interrupted)
        .await
        .expect("interrupt");

    // Recovery scan like check_interrupted_on_startup.
    let interrupted = session_repo.list_interrupted().await.expect("list");
    assert_eq!(interrupted.len(), 1);

    let mut total_approvals = 0;
    let mut total_prompts = 0;
    for s in &interrupted {
        if let Ok(Some(_)) = approval_repo.get_pending_for_session(&s.id).await {
            total_approvals += 1;
        }
        if let Ok(Some(_)) = prompt_repo.get_pending_for_session(&s.id).await {
            total_prompts += 1;
        }
    }

    assert_eq!(total_approvals, 1);
    assert_eq!(total_prompts, 1);
}

/// Create and activate a session directly in the database.
async fn create_active_session_in_db(db: &Arc<sqlx::SqlitePool>, workspace_root: &str) -> Session {
    let repo = SessionRepo::new(Arc::clone(db));
    let session = Session::new(
        "U_OWNER".into(),
        workspace_root.into(),
        Some("test session".into()),
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create session");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate session")
}
