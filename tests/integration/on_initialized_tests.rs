//! Integration tests for `AgentRcServer::on_initialized` auto-session
//! creation and spawned-agent session verification.
//!
//! Validates:
//! - Direct connection → auto-creates and activates session
//! - Spawned agent with valid `session_id` → finds existing session
//! - Spawned agent with invalid `session_id` → logs warning, no crash
//! - Direct connection in remote mode → session has Remote mode
//! - Direct connection in local mode → session has Local mode
//! - Auto-created session uses config `default_workspace_root`
//! - Direct connection terminates stale active direct-connection sessions
//!
//! # Coverage note
//!
//! `AgentRcServer::on_initialized` cannot be invoked directly in tests because
//! `NotificationContext<RoleServer>` requires a live MCP transport to construct.
//! These tests verify the constituent repository operations that `on_initialized`
//! delegates to (session creation, status update, stale-cleanup) rather than the
//! method's control flow itself.  A logic regression in `on_initialized` (e.g.,
//! dropping the stale-cleanup branch) would not be caught here.  If direct
//! invocation becomes necessary, consider extracting the inner async logic into a
//! `pub(crate) async fn initialize_session(state, session_id_override, is_remote)`
//! helper that both `on_initialized` and these tests can call.

use std::collections::HashMap;
use std::sync::Arc;

use monocoque_agent_rc::mcp::handler::{AgentRcServer, AppState};
use monocoque_agent_rc::models::session::{Session, SessionMode, SessionStatus};
use monocoque_agent_rc::persistence::db;
use monocoque_agent_rc::persistence::session_repo::SessionRepo;
use tokio::sync::Mutex;

use super::test_helpers::test_config;

// ── on_initialized: direct connection creates session ────────

#[tokio::test]
async fn on_initialized_direct_connection_creates_session() {
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
        stall_detectors: None,
        ipc_auth_token: None,
    });

    // Direct connection: no session_id_override, no channel_id_override.
    // Simulate what on_initialized does.
    let session_repo = SessionRepo::new(Arc::clone(&database));

    let workspace_root = state
        .config
        .default_workspace_root()
        .to_string_lossy()
        .into_owned();
    let mode = SessionMode::Local; // No channel override → local.

    let session = Session::new(
        "agent:local".into(),
        workspace_root.clone(),
        Some("Direct agent connection".into()),
        mode,
    );
    let created = session_repo.create(&session).await.expect("create");
    assert_eq!(created.status, SessionStatus::Created);

    let activated = session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");
    assert_eq!(activated.status, SessionStatus::Active);

    // Verify it's discoverable.
    let active_list = session_repo.list_active().await.expect("list");
    assert_eq!(active_list.len(), 1);
    assert_eq!(active_list[0].id, created.id);
    assert_eq!(active_list[0].mode, SessionMode::Local);
}

// ── on_initialized: remote connection creates Remote session ─

#[tokio::test]
async fn on_initialized_remote_connection_creates_remote_session() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let session_repo = SessionRepo::new(Arc::clone(&database));

    // With channel_id → is_remote = true → Remote mode.
    let workspace_root = root.to_string();
    let mode = SessionMode::Remote;

    let session = Session::new(
        "agent:local".into(),
        workspace_root,
        Some("Direct agent connection".into()),
        mode,
    );
    let created = session_repo.create(&session).await.expect("create");
    session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate");

    let active = session_repo.list_active().await.expect("list");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].mode, SessionMode::Remote);
}

// ── on_initialized: spawned agent finds pre-created session ──

#[tokio::test]
async fn on_initialized_spawned_agent_finds_session() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let session_repo = SessionRepo::new(Arc::clone(&database));

    // Pre-create and activate session (simulating /spawn command).
    let session = Session::new(
        "U_SPAWNER".into(),
        root.into(),
        Some("Build feature X".into()),
        SessionMode::Remote,
    );
    let pre_created = session_repo.create(&session).await.expect("create");
    session_repo
        .update_status(&pre_created.id, SessionStatus::Active)
        .await
        .expect("activate");

    // Spawned agent connects with session_id_override.
    let session_id_override = Some(pre_created.id.clone());

    // on_initialized verifies the session exists.
    if let Some(ref sid) = session_id_override {
        let found = session_repo
            .get_by_id(sid)
            .await
            .expect("query")
            .expect("found");
        assert_eq!(found.id, pre_created.id);
        assert_eq!(found.status, SessionStatus::Active);
        assert_eq!(found.owner_user_id, "U_SPAWNER");
    }
}

// ── on_initialized: spawned agent with invalid session_id ────

#[tokio::test]
async fn on_initialized_invalid_session_id_no_crash() {
    let temp = tempfile::tempdir().expect("tempdir");
    let _root = temp.path().to_str().expect("utf8");
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let session_repo = SessionRepo::new(Arc::clone(&database));

    let session_id_override = Some("nonexistent-session".to_string());

    if let Some(ref sid) = session_id_override {
        let result = session_repo.get_by_id(sid).await;
        match result {
            Ok(None) => {} // Expected: session not found → log warning.
            Ok(Some(_)) => panic!("should not find nonexistent session"),
            Err(e) => panic!("query should succeed even for missing session: {e}"),
        }
    }
}

// ── on_initialized: auto-created session uses default root ───

#[tokio::test]
async fn on_initialized_uses_default_workspace_root() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let config = test_config(root);

    let expected_root = config
        .default_workspace_root()
        .to_string_lossy()
        .into_owned();

    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let session_repo = SessionRepo::new(Arc::clone(&database));

    let session = Session::new(
        "agent:local".into(),
        expected_root.clone(),
        Some("Direct agent connection".into()),
        SessionMode::Local,
    );
    let created = session_repo.create(&session).await.expect("create");

    assert_eq!(created.workspace_root, expected_root);
}

// ── on_initialized: server constructors produce correct overrides ──

#[tokio::test]
async fn server_constructors_correct_overrides() {
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
        stall_detectors: None,
        ipc_auth_token: None,
    });

    // new() — no overrides.
    let server_plain = AgentRcServer::new(Arc::clone(&state));
    assert_eq!(server_plain.effective_channel_id(), Some("C_TEST"));

    // with_channel_override — channel set.
    let server_ch =
        AgentRcServer::with_channel_override(Arc::clone(&state), Some("C_CUSTOM".into()));
    assert_eq!(server_ch.effective_channel_id(), Some("C_CUSTOM"));

    // with_overrides — both channel and session.
    let server_full = AgentRcServer::with_overrides(
        Arc::clone(&state),
        Some("C_FULL".into()),
        Some("session-123".into()),
    );
    assert_eq!(server_full.effective_channel_id(), Some("C_FULL"));
}

// ── on_initialized: direct connection terminates stale sessions ──

#[tokio::test]
async fn on_initialized_direct_connection_terminates_stale_sessions() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let session_repo = SessionRepo::new(Arc::clone(&database));

    // Pre-create two active direct-connection sessions (simulating
    // prior window reloads that left stale sessions behind).
    let stale_1 = Session::new(
        "agent:local".into(),
        root.into(),
        Some("Direct agent connection".into()),
        SessionMode::Local,
    );
    let stale_1 = session_repo.create(&stale_1).await.expect("create stale_1");
    session_repo
        .update_status(&stale_1.id, SessionStatus::Active)
        .await
        .expect("activate stale_1");

    let stale_2 = Session::new(
        "agent:local".into(),
        root.into(),
        Some("Direct agent connection".into()),
        SessionMode::Remote,
    );
    let stale_2 = session_repo.create(&stale_2).await.expect("create stale_2");
    session_repo
        .update_status(&stale_2.id, SessionStatus::Active)
        .await
        .expect("activate stale_2");

    // Verify there are now two active sessions.
    let active = session_repo.list_active().await.expect("list");
    assert_eq!(active.len(), 2, "expected 2 stale active sessions");

    // Simulate what on_initialized Case 2 does (with stale cleanup).
    // Terminate stale direct-connection sessions before creating a new one.
    let stale_sessions = session_repo.list_active().await.expect("list active");
    for stale in &stale_sessions {
        if stale.owner_user_id == "agent:local" {
            session_repo
                .set_terminated(&stale.id, SessionStatus::Terminated)
                .await
                .expect("terminate stale");
        }
    }

    // Create and activate the new session.
    let new_session = Session::new(
        "agent:local".into(),
        root.into(),
        Some("Direct agent connection".into()),
        SessionMode::Local,
    );
    let created = session_repo.create(&new_session).await.expect("create new");
    session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate new");

    // Exactly one active session should remain — the newly created one.
    let active = session_repo.list_active().await.expect("list");
    assert_eq!(
        active.len(),
        1,
        "expected exactly 1 active session after cleanup"
    );
    assert_eq!(active[0].id, created.id);

    // Stale sessions should now be terminated.
    let s1 = session_repo
        .get_by_id(&stale_1.id)
        .await
        .expect("query")
        .expect("found");
    assert_eq!(s1.status, SessionStatus::Terminated);

    let s2 = session_repo
        .get_by_id(&stale_2.id)
        .await
        .expect("query")
        .expect("found");
    assert_eq!(s2.status, SessionStatus::Terminated);
}

// ── on_initialized: spawned-agent sessions are NOT terminated ──

#[tokio::test]
async fn on_initialized_direct_connection_does_not_terminate_spawned_sessions() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let session_repo = SessionRepo::new(Arc::clone(&database));

    // Pre-create an active spawned session (owner is a real user, not "agent:local").
    let spawned = Session::new(
        "U_REAL_USER".into(),
        root.into(),
        Some("Build feature X".into()),
        SessionMode::Remote,
    );
    let spawned = session_repo.create(&spawned).await.expect("create spawned");
    session_repo
        .update_status(&spawned.id, SessionStatus::Active)
        .await
        .expect("activate spawned");

    // Simulate the stale cleanup (only targets "agent:local" sessions).
    let stale_sessions = session_repo.list_active().await.expect("list active");
    for stale in &stale_sessions {
        if stale.owner_user_id == "agent:local" {
            session_repo
                .set_terminated(&stale.id, SessionStatus::Terminated)
                .await
                .expect("terminate stale");
        }
    }

    // Create and activate new direct-connection session.
    let new_session = Session::new(
        "agent:local".into(),
        root.into(),
        Some("Direct agent connection".into()),
        SessionMode::Local,
    );
    let created = session_repo.create(&new_session).await.expect("create new");
    session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate new");

    // Two active sessions: the spawned one and the new direct one.
    let active = session_repo.list_active().await.expect("list");
    assert_eq!(
        active.len(),
        2,
        "spawned session must survive stale cleanup"
    );

    // Spawned session is still active.
    let s = session_repo
        .get_by_id(&spawned.id)
        .await
        .expect("query")
        .expect("found");
    assert_eq!(s.status, SessionStatus::Active);
}
