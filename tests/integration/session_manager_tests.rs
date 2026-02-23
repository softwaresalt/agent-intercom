//! Integration tests for session manager orchestrator functions.
//!
//! Validates `pause_session`, `resume_session`, `terminate_session`,
//! and `resolve_session` through the orchestrator module.

use std::sync::Arc;

use monocoque_agent_rc::models::session::{Session, SessionMode, SessionStatus};
use monocoque_agent_rc::orchestrator::session_manager;
use monocoque_agent_rc::orchestrator::spawner;
use monocoque_agent_rc::persistence::db;
use monocoque_agent_rc::persistence::session_repo::SessionRepo;

// ── Pause active session ─────────────────────────────────────

#[tokio::test]
async fn pause_session_sets_status_to_paused() {
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let repo = SessionRepo::new(Arc::clone(&database));

    let session = Session::new(
        "U_OWNER".into(),
        "/test/ws".into(),
        None,
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let paused = session_manager::pause_session(&created.id, &repo)
        .await
        .expect("pause");
    assert_eq!(paused.status, SessionStatus::Paused);
}

// ── Resume paused session ────────────────────────────────────

#[tokio::test]
async fn resume_session_reactivates_paused() {
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let repo = SessionRepo::new(Arc::clone(&database));

    let session = Session::new(
        "U_OWNER".into(),
        "/test/ws".into(),
        None,
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    session_manager::pause_session(&created.id, &repo)
        .await
        .expect("pause");

    let resumed = session_manager::resume_session(&created.id, &repo)
        .await
        .expect("resume");
    assert_eq!(resumed.status, SessionStatus::Active);
}

// ── Terminate active session (no child process) ──────────────

#[tokio::test]
async fn terminate_session_without_child_process() {
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let repo = SessionRepo::new(Arc::clone(&database));

    let session = Session::new(
        "U_OWNER".into(),
        "/test/ws".into(),
        None,
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let terminated = session_manager::terminate_session(&created.id, &repo, None)
        .await
        .expect("terminate");
    assert_eq!(terminated.status, SessionStatus::Terminated);
    assert!(terminated.terminated_at.is_some());
}

// ── Invalid transition: terminate → resume ───────────────────

#[tokio::test]
async fn resume_terminated_session_fails() {
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let repo = SessionRepo::new(Arc::clone(&database));

    let session = Session::new(
        "U_OWNER".into(),
        "/test/ws".into(),
        None,
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    session_manager::terminate_session(&created.id, &repo, None)
        .await
        .expect("terminate");

    let result = session_manager::resume_session(&created.id, &repo).await;
    assert!(result.is_err(), "resuming terminated session should fail");
}

// ── Invalid transition: created → pause ──────────────────────

#[tokio::test]
async fn pause_created_session_fails() {
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let repo = SessionRepo::new(Arc::clone(&database));

    let session = Session::new(
        "U_OWNER".into(),
        "/test/ws".into(),
        None,
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create");

    // Session is Created, not Active — pause should fail.
    let result = session_manager::pause_session(&created.id, &repo).await;
    assert!(result.is_err(), "pausing created session should fail");
}

// ── resolve_session: find by user's active session ───────────

#[tokio::test]
async fn resolve_session_by_active_user() {
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let repo = SessionRepo::new(Arc::clone(&database));

    let session = Session::new(
        "U_OWNER".into(),
        "/test/ws".into(),
        None,
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let resolved = session_manager::resolve_session(None, "U_OWNER", &repo)
        .await
        .expect("resolve");
    assert_eq!(resolved.id, created.id);
}

// ── resolve_session: by explicit ID ──────────────────────────

#[tokio::test]
async fn resolve_session_by_explicit_id() {
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let repo = SessionRepo::new(Arc::clone(&database));

    let session = Session::new(
        "U_OWNER".into(),
        "/test/ws".into(),
        None,
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let resolved = session_manager::resolve_session(Some(&created.id), "U_OWNER", &repo)
        .await
        .expect("resolve");
    assert_eq!(resolved.id, created.id);
}

// ── resolve_session: wrong user gets unauthorized ────────────

#[tokio::test]
async fn resolve_session_wrong_user_unauthorized() {
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let repo = SessionRepo::new(Arc::clone(&database));

    let session = Session::new(
        "U_OWNER".into(),
        "/test/ws".into(),
        None,
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let result = session_manager::resolve_session(Some(&created.id), "U_IMPOSTER", &repo).await;
    assert!(result.is_err(), "wrong user should be unauthorized");
}

// ── resolve_session: no active session → not found ───────────

#[tokio::test]
async fn resolve_session_no_active_not_found() {
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let repo = SessionRepo::new(Arc::clone(&database));

    let result = session_manager::resolve_session(None, "U_NOBODY", &repo).await;
    assert!(result.is_err(), "no active session should be not found");
}

// ── verify_session_owner ─────────────────────────────────────

#[tokio::test]
async fn verify_session_owner_succeeds_for_owner() {
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let repo = SessionRepo::new(Arc::clone(&database));

    let session = Session::new(
        "U_OWNER".into(),
        "/test/ws".into(),
        None,
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create");

    let result = spawner::verify_session_owner(&created, "U_OWNER");
    assert!(result.is_ok());
}

#[tokio::test]
async fn verify_session_owner_fails_for_other() {
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let repo = SessionRepo::new(Arc::clone(&database));

    let session = Session::new(
        "U_OWNER".into(),
        "/test/ws".into(),
        None,
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create");

    let result = spawner::verify_session_owner(&created, "U_OTHER");
    assert!(result.is_err());
}

// ── Full lifecycle: pause → resume → terminate ───────────────

#[tokio::test]
async fn full_orchestrator_lifecycle() {
    let database = Arc::new(db::connect_memory().await.expect("db"));
    let repo = SessionRepo::new(Arc::clone(&database));

    let session = Session::new(
        "U_OWNER".into(),
        "/test/ws".into(),
        Some("test lifecycle".into()),
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create");
    let active = repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");
    assert_eq!(active.status, SessionStatus::Active);

    let paused = session_manager::pause_session(&created.id, &repo)
        .await
        .expect("pause");
    assert_eq!(paused.status, SessionStatus::Paused);

    let resumed = session_manager::resume_session(&created.id, &repo)
        .await
        .expect("resume");
    assert_eq!(resumed.status, SessionStatus::Active);

    let terminated = session_manager::terminate_session(&created.id, &repo, None)
        .await
        .expect("terminate");
    assert_eq!(terminated.status, SessionStatus::Terminated);
    assert!(terminated.terminated_at.is_some());
}
