//! Shared test helpers for handler-level integration tests.
//!
//! Provides reusable construction of `AppState`, `GlobalConfig`,
//! active sessions, and other prerequisites so individual test
//! modules can focus on behaviour rather than boilerplate.

use std::collections::HashMap;
use std::sync::Arc;

use agent_intercom::config::GlobalConfig;
use agent_intercom::mcp::handler::{AppState, IntercomServer};
use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::persistence::db;
use agent_intercom::persistence::session_repo::SessionRepo;
use sqlx::SqlitePool;
use tokio::sync::Mutex;

/// Build a minimal `GlobalConfig` pointing at the given `workspace_root`
/// with sensible defaults for test isolation.
pub fn test_config(workspace_root: &str) -> GlobalConfig {
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 0
ipc_name = "test-handler"
max_concurrent_sessions = 5
host_cli = "echo"

[slack]
channel_id = "C_TEST"

[timeouts]
approval_seconds = 2
prompt_seconds = 2
wait_seconds = 2

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#,
        root = workspace_root.replace('\\', "\\\\"),
    );
    GlobalConfig::from_toml_str(&toml).expect("valid test config")
}

/// Build a minimal `GlobalConfig` pointing at the given `workspace_root`
/// with an empty Slack channel (simulating no-Slack-channel mode).
pub fn test_config_no_channel(workspace_root: &str) -> GlobalConfig {
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 0
ipc_name = "test-handler-no-ch"
max_concurrent_sessions = 5
host_cli = "echo"

[slack]

[timeouts]
approval_seconds = 2
prompt_seconds = 2
wait_seconds = 2

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#,
        root = workspace_root.replace('\\', "\\\\"),
    );
    GlobalConfig::from_toml_str(&toml).expect("valid test config")
}

/// Build a complete `AppState` with in-memory `SQLite` and no Slack client.
pub async fn test_app_state(config: GlobalConfig) -> Arc<AppState> {
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    Arc::new(AppState {
        config: Arc::new(config),
        db: database,
        slack: None,
        pending_approvals: Arc::new(Mutex::new(HashMap::new())),
        pending_prompts: Arc::new(Mutex::new(HashMap::new())),
        pending_waits: Arc::new(Mutex::new(HashMap::new())),
        pending_modal_contexts: Default::default(),
        stall_detectors: None,
        ipc_auth_token: None,
    })
}

/// Build a complete `AppState` with an already-created in-memory DB pool.
#[allow(dead_code)]
pub fn test_app_state_with_db(config: GlobalConfig, db: Arc<SqlitePool>) -> Arc<AppState> {
    Arc::new(AppState {
        config: Arc::new(config),
        db,
        slack: None,
        pending_approvals: Arc::new(Mutex::new(HashMap::new())),
        pending_prompts: Arc::new(Mutex::new(HashMap::new())),
        pending_waits: Arc::new(Mutex::new(HashMap::new())),
        pending_modal_contexts: Default::default(),
        stall_detectors: None,
        ipc_auth_token: None,
    })
}

/// Create an `IntercomServer` with a channel override.
#[allow(dead_code)]
pub fn test_server(state: Arc<AppState>, channel_id: Option<&str>) -> IntercomServer {
    IntercomServer::with_channel_override(state, channel_id.map(String::from))
}

/// Create and activate a session in the database, returning the active session.
pub async fn create_active_session(db: &Arc<SqlitePool>, workspace_root: &str) -> Session {
    let repo = SessionRepo::new(Arc::clone(db));
    let session = Session::new(
        "U_TEST_OWNER".into(),
        workspace_root.into(),
        Some("test session".into()),
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create session");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate session")
}

/// Create and activate a session with a specific mode.
pub async fn create_active_session_with_mode(
    db: &Arc<SqlitePool>,
    workspace_root: &str,
    mode: SessionMode,
) -> Session {
    let repo = SessionRepo::new(Arc::clone(db));
    let session = Session::new(
        "U_TEST_OWNER".into(),
        workspace_root.into(),
        Some("test session".into()),
        mode,
    );
    let created = repo.create(&session).await.expect("create session");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate session")
}

/// Create an interrupted session in the database for recovery tests.
pub async fn create_interrupted_session(db: &Arc<SqlitePool>, workspace_root: &str) -> Session {
    let repo = SessionRepo::new(Arc::clone(db));
    let session = Session::new(
        "U_TEST_OWNER".into(),
        workspace_root.into(),
        Some("interrupted session".into()),
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create session");
    let active = repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate session");
    repo.update_status(&active.id, SessionStatus::Interrupted)
        .await
        .expect("interrupt session")
}
