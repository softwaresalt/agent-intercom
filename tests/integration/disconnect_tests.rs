//! Integration tests for SSE / stdio disconnect session cleanup (US5).
//!
//! Validates that when an `IntercomServer` instance is dropped — which happens
//! when the MCP transport closes — the associated DB session is marked
//! `Terminated` so stale "Active" sessions no longer accumulate.
//!
//! # Scenarios covered
//!
//! | ID   | Scenario |
//! |------|----------|
//! | S037 | Direct-connection server drop marks active session as Terminated |
//! | S038 | Spawned-agent server drop leaves the session unchanged (no-op) |
//! | S039 | Fresh server drop with no session ID set is a safe no-op |
//!
//! # Note on `set_session_id_for_testing`
//!
//! `IntercomServer::on_initialized` cannot be called in tests because it
//! requires a live MCP `NotificationContext`.  These tests instead use
//! `set_session_id_for_testing()` to inject the DB session ID that
//! `on_initialized` would normally store, then `drop()` the server to trigger
//! the same cleanup path exercised in production.

use std::collections::HashMap;
use std::sync::Arc;

use agent_intercom::mcp::handler::{AppState, IntercomServer};
use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::persistence::db;
use agent_intercom::persistence::session_repo::SessionRepo;
use tokio::sync::Mutex;

use super::test_helpers::test_config;

// ── S037: drop marks active direct-connection session as Terminated ──────────

/// S037 — When a direct-connection `IntercomServer` is dropped, the active
/// session it owns should be marked [`SessionStatus::Terminated`].
#[tokio::test]
async fn drop_marks_direct_session_terminated() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let config = test_config(root);
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let state = Arc::new(AppState {
        config: Arc::new(config),
        db: Arc::clone(&database),
        slack: None,
        pending_approvals: Arc::new(Mutex::new(HashMap::new())),
        pending_prompts: Arc::new(Mutex::new(HashMap::new())),
        pending_waits: Arc::new(Mutex::new(HashMap::new())),
        pending_modal_contexts: Arc::default(),
        stall_detectors: None,
        ipc_auth_token: None,
        policy_cache: Arc::default(),
        audit_logger: None,
        active_children: Arc::default(),
    });

    // Create and activate a local session.
    let session_repo = SessionRepo::new(Arc::clone(&database));
    let session = Session::new(
        "agent:local".to_owned(),
        root.to_owned(),
        None,
        SessionMode::Local,
    );
    let created = session_repo.create(&session).await.expect("create session");
    session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate session");

    // Simulate what `on_initialized` does for a direct connection: store the
    // session ID in the server so Drop can clean it up.
    let server = IntercomServer::new(Arc::clone(&state));
    server.set_session_id_for_testing(created.id.clone());

    // Drop the server (simulates transport close / agent disconnect).
    drop(server);

    // Give the spawned cleanup task a moment to write to the DB.
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    // The session must now be Terminated.
    let fetched = session_repo
        .get_by_id(&created.id)
        .await
        .expect("get session")
        .expect("session must exist");
    assert_eq!(
        fetched.status,
        SessionStatus::Terminated,
        "dropped direct-connection session must be Terminated"
    );
}

// ── S038: spawned-agent drop is a no-op ──────────────────────────────────────

/// S038 — A spawned-agent `IntercomServer` (created via `with_overrides` and a
/// `session_id_override`) never stores a session ID in `session_db_id` because
/// `on_initialized` returns early from Case 1.  Dropping such a server must
/// leave the pre-existing session untouched.
#[tokio::test]
async fn drop_spawned_server_does_not_terminate_session() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let config = test_config(root);
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let state = Arc::new(AppState {
        config: Arc::new(config),
        db: Arc::clone(&database),
        slack: None,
        pending_approvals: Arc::new(Mutex::new(HashMap::new())),
        pending_prompts: Arc::new(Mutex::new(HashMap::new())),
        pending_waits: Arc::new(Mutex::new(HashMap::new())),
        pending_modal_contexts: Arc::default(),
        stall_detectors: None,
        ipc_auth_token: None,
        policy_cache: Arc::default(),
        audit_logger: None,
        active_children: Arc::default(),
    });

    // Create and activate a remote (spawned) session.
    let session_repo = SessionRepo::new(Arc::clone(&database));
    let session = Session::new(
        "U12345678".to_owned(),
        root.to_owned(),
        Some("Spawned session".to_owned()),
        SessionMode::Remote,
    );
    let created = session_repo.create(&session).await.expect("create session");
    session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate session");

    // Spawned server: session_id_override is set, so `on_initialized` would
    // return early from Case 1 without calling `set_session_id_for_testing`.
    let server = IntercomServer::with_overrides(Arc::clone(&state), None, Some(created.id.clone()));
    // Intentionally do NOT call set_session_id_for_testing — mirrors Case 1.
    drop(server);

    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    // Session must still be Active — the Drop should be a no-op.
    let fetched = session_repo
        .get_by_id(&created.id)
        .await
        .expect("get session")
        .expect("session must exist");
    assert_eq!(
        fetched.status,
        SessionStatus::Active,
        "spawned-agent session must remain Active after server drop"
    );
}

// ── S039: fresh-server drop with no session is a safe no-op ──────────────────

/// S039 — An `IntercomServer` that was dropped before `on_initialized` ran
/// (e.g., transport handshake failure) must not panic or error, and the DB
/// must remain unchanged.
#[tokio::test]
async fn drop_fresh_server_with_no_session_is_noop() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let config = test_config(root);
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let state = Arc::new(AppState {
        config: Arc::new(config),
        db: Arc::clone(&database),
        slack: None,
        pending_approvals: Arc::new(Mutex::new(HashMap::new())),
        pending_prompts: Arc::new(Mutex::new(HashMap::new())),
        pending_waits: Arc::new(Mutex::new(HashMap::new())),
        pending_modal_contexts: Arc::default(),
        stall_detectors: None,
        ipc_auth_token: None,
        policy_cache: Arc::default(),
        audit_logger: None,
        active_children: Arc::default(),
    });

    // Drop an uninitialized server — no session ID set.
    let server = IntercomServer::new(Arc::clone(&state));
    drop(server);

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // No sessions should exist — nothing was created or modified.
    let session_repo = SessionRepo::new(Arc::clone(&database));
    let active = session_repo.list_active().await.expect("list active");
    assert!(
        active.is_empty(),
        "no sessions should exist after a no-op drop"
    );
}
