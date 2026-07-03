//! Unit tests for `/arc queue` slash command handling.

use std::collections::HashMap;
use std::sync::Arc;

use agent_intercom::config::GlobalConfig;
use agent_intercom::driver::mcp_driver::McpDriver;
use agent_intercom::mcp::handler::AppState;
use agent_intercom::mode::ServerMode;
use agent_intercom::persistence::db;
use agent_intercom::slack::commands::dispatch_command;
use tokio::sync::Mutex;

fn make_config(workspace_root: &str, user: &str) -> GlobalConfig {
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 0
ipc_name = "test-intercom-queue"
max_concurrent_sessions = 5
host_cli = "echo"

[slack]
channel_id = "C_TEST"

[timeouts]
approval_seconds = 5
prompt_seconds = 5
wait_seconds = 5

[stall]
enabled = false
inactivity_threshold_seconds = 300
escalation_threshold_seconds = 120
max_retries = 3
default_nudge_message = "continue"
"#,
        root = workspace_root.replace('\\', "\\\\"),
    );
    let mut config = GlobalConfig::from_toml_str(&toml).expect("valid test config");
    config.authorized_user_ids = vec![user.to_owned()];
    config
}

async fn app_state_with_mode(
    workspace_root: &str,
    user: &str,
    server_mode: ServerMode,
) -> Arc<AppState> {
    let config = make_config(workspace_root, user);
    let database = Arc::new(db::connect_memory().await.expect("db connect"));

    Arc::new(AppState {
        config: Arc::new(config),
        db: Arc::clone(&database),
        slack: None,
        pending_approvals: Arc::new(Mutex::new(HashMap::new())),
        pending_prompts: Arc::new(Mutex::new(HashMap::new())),
        pending_waits: Arc::new(Mutex::new(HashMap::new())),
        pending_modal_contexts: Arc::default(),
        pending_thread_replies: Arc::default(),
        stall_detectors: None,
        ipc_auth_token: None,
        policy_cache: Arc::default(),
        audit_logger: None,
        active_children: Arc::default(),
        pending_command_approvals: Arc::default(),
        stall_event_tx: None,
        driver: McpDriver::new_empty(),
        server_mode,
        workspace_mappings: Arc::default(),
        acp_event_tx: None,
        acp_driver: None,
    })
}

#[tokio::test]
async fn queue_rejected_in_mcp_mode() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";
    let state = app_state_with_mode(root, user, ServerMode::Mcp).await;

    let result = dispatch_command("queue", &["list"], user, "C_TEST", &state).await;

    let message = result.expect("queue response");
    assert!(message.contains("only available in ACP mode"));
}

#[tokio::test]
async fn acp_help_includes_queue_commands() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";
    let state = app_state_with_mode(root, user, ServerMode::Acp).await;

    let result = dispatch_command("help", &[], user, "C_TEST", &state).await;

    let message = result.expect("help response");
    assert!(
        message.contains("queue add"),
        "help missing queue add: {message}"
    );
    assert!(
        message.contains("queue transfer"),
        "help missing queue transfer: {message}"
    );
}

#[tokio::test]
async fn queue_add_list_and_replace_round_trip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";
    let state = app_state_with_mode(root, user, ServerMode::Acp).await;

    dispatch_command("queue", &["add", "alpha"], user, "C_TEST", &state)
        .await
        .expect("add response");

    let listed = dispatch_command("queue", &["list"], user, "C_TEST", &state)
        .await
        .expect("list response");
    assert_eq!(listed, "1. alpha");

    dispatch_command("queue", &["replace", "1", "beta"], user, "C_TEST", &state)
        .await
        .expect("replace response");

    let replaced = dispatch_command("queue", &["list"], user, "C_TEST", &state)
        .await
        .expect("list response after replace");
    assert_eq!(replaced, "1. beta");
}

#[tokio::test]
async fn queue_list_empty_returns_empty_message() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";
    let state = app_state_with_mode(root, user, ServerMode::Acp).await;

    let result = dispatch_command("queue", &["list"], user, "C_TEST", &state).await;

    let message = result.expect("list response");
    assert_eq!(message, "Queue is empty.");
}

#[tokio::test]
async fn queue_list_multiple_items_uses_newline_delimiter() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";
    let state = app_state_with_mode(root, user, ServerMode::Acp).await;

    dispatch_command("queue", &["add", "alpha"], user, "C_TEST", &state)
        .await
        .expect("add alpha");
    dispatch_command("queue", &["add", "beta"], user, "C_TEST", &state)
        .await
        .expect("add beta");

    let listed = dispatch_command("queue", &["list"], user, "C_TEST", &state)
        .await
        .expect("list response");
    // Items are joined by a bare newline (no leading space on continuation lines).
    assert_eq!(listed, "1. alpha\n2. beta");
}

#[tokio::test]
async fn queue_transfer_requires_number() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";
    let state = app_state_with_mode(root, user, ServerMode::Acp).await;

    let result = dispatch_command("queue", &["transfer"], user, "C_TEST", &state).await;

    let err = result.expect_err("transfer without a number should error");
    assert!(
        format!("{err:?}").contains("usage: queue transfer"),
        "unexpected error: {err:?}"
    );
}

#[tokio::test]
async fn queue_transfer_rejects_non_numeric() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";
    let state = app_state_with_mode(root, user, ServerMode::Acp).await;

    let result = dispatch_command("queue", &["transfer", "abc"], user, "C_TEST", &state).await;

    assert!(
        result.is_err(),
        "a non-numeric transfer argument should error before invoking backlogit"
    );
}

#[tokio::test]
async fn queue_transfer_missing_item_errors() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";
    let state = app_state_with_mode(root, user, ServerMode::Acp).await;

    // Empty queue: transfer resolves the item before shelling out to backlogit,
    // so a missing item errors without any side effect.
    let result = dispatch_command("queue", &["transfer", "99"], user, "C_TEST", &state).await;

    let err = result.expect_err("transfer of a missing item should error");
    assert!(
        format!("{err:?}").contains("not found"),
        "unexpected error: {err:?}"
    );
}
