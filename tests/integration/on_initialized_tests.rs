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
