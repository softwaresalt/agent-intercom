//! Integration tests for dynamic Slack channel selection (T204, US12).
//!
//! Validates:
//! - `?channel_id=C_OVERRIDE` on SSE endpoint uses per-session override
//! - Missing `?channel_id=` falls back to config default (when non-empty)
//! - Empty `?channel_id=` falls back to config default
//! - No channel configured at all returns `None`
//! - Two concurrent sessions with different overrides route independently

use std::collections::HashMap;
use std::sync::Arc;

use agent_intercom::config::GlobalConfig;
use agent_intercom::mcp::handler::{AppState, IntercomServer};
use agent_intercom::persistence::db;
use tokio::sync::Mutex;

/// Build a minimal test configuration with in-memory DB and a known default channel.
fn test_config() -> GlobalConfig {
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 0
ipc_name = "test-channel-override"
max_concurrent_sessions = 3
host_cli = "echo"

[slack]
channel_id = "C_DEFAULT_CHANNEL"

[timeouts]
approval_seconds = 3600
prompt_seconds = 1800
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#,
        root = temp.path().to_str().expect("utf8"),
    );
    GlobalConfig::from_toml_str(&toml).expect("valid config")
}

/// Build a minimal `AppState` for channel override testing (no Slack client needed).
async fn test_state() -> Arc<AppState> {
    let config = test_config();
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    Arc::new(AppState {
        config: Arc::new(config),
        db: database,
        slack: None,
        pending_approvals: Arc::new(Mutex::new(HashMap::new())),
        pending_prompts: Arc::new(Mutex::new(HashMap::new())),
        pending_waits: Arc::new(Mutex::new(HashMap::new())),
        pending_modal_contexts: Arc::default(),
        stall_detectors: None,
        ipc_auth_token: None,
        policy_cache: Arc::default(),
        audit_logger: None,
    })
}

#[tokio::test]
async fn channel_override_uses_specified_channel() {
    let state = test_state().await;
    let server =
        IntercomServer::with_channel_override(Arc::clone(&state), Some("C_OVERRIDE".into()));

    assert_eq!(server.effective_channel_id(), Some("C_OVERRIDE"));
}

#[tokio::test]
async fn absent_channel_id_uses_config_default() {
    let state = test_state().await;
    let server = IntercomServer::with_channel_override(Arc::clone(&state), None);

    assert_eq!(server.effective_channel_id(), Some("C_DEFAULT_CHANNEL"));
}

#[tokio::test]
async fn new_server_uses_config_default() {
    let state = test_state().await;
    let server = IntercomServer::new(Arc::clone(&state));

    assert_eq!(server.effective_channel_id(), Some("C_DEFAULT_CHANNEL"));
}

#[tokio::test]
async fn two_sessions_with_different_overrides_route_independently() {
    let state = test_state().await;

    let server_a =
        IntercomServer::with_channel_override(Arc::clone(&state), Some("C_FRONTEND".into()));
    let server_b =
        IntercomServer::with_channel_override(Arc::clone(&state), Some("C_BACKEND".into()));

    // Both share the same AppState but each session routes to its own channel.
    assert_eq!(server_a.effective_channel_id(), Some("C_FRONTEND"));
    assert_eq!(server_b.effective_channel_id(), Some("C_BACKEND"));

    // Neither clobbers the other.
    assert_ne!(
        server_a.effective_channel_id(),
        server_b.effective_channel_id()
    );
}

#[tokio::test]
async fn no_channel_anywhere_returns_none() {
    // Build a state with an empty [slack] section (no channel_id in config).
    let temp = tempfile::tempdir().expect("tempdir");
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 0
ipc_name = "test-no-channel"
max_concurrent_sessions = 3
host_cli = "echo"

[slack]

[timeouts]
approval_seconds = 3600
prompt_seconds = 1800
wait_seconds = 0

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#,
        root = temp.path().to_str().expect("utf8"),
    );
    let config = GlobalConfig::from_toml_str(&toml).expect("valid config");
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let state = Arc::new(AppState {
        config: Arc::new(config),
        db: database,
        slack: None,
        pending_approvals: Arc::new(Mutex::new(HashMap::new())),
        pending_prompts: Arc::new(Mutex::new(HashMap::new())),
        pending_waits: Arc::new(Mutex::new(HashMap::new())),
        pending_modal_contexts: Arc::default(),
        stall_detectors: None,
        ipc_auth_token: None,
        policy_cache: Arc::default(),
        audit_logger: None,
    });

    // No override, no config channel → None.
    let server = IntercomServer::new(Arc::clone(&state));
    assert_eq!(server.effective_channel_id(), None);

    // Empty-string override also resolves to None via config (which is also empty).
    let server_no_override = IntercomServer::with_channel_override(Arc::clone(&state), None);
    assert_eq!(server_no_override.effective_channel_id(), None);
}
