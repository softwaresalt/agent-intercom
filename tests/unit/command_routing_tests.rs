//! Unit tests for slash command routing and mode gating (Phase 2, Task 2.4).
//!
//! Tests the `dispatch_command` function that routes parsed command tokens
//! to the correct handler based on server mode. Scenarios covered:
//! - S-T1-021: Malformed arguments → descriptive usage message
//! - S-T1-022: MCP mode accepts valid commands (steer)
//! - S-T1-023: ACP-only commands are rejected in MCP mode with mode-mismatch message

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use agent_intercom::config::GlobalConfig;
use agent_intercom::driver::mcp_driver::McpDriver;
use agent_intercom::mcp::handler::AppState;
use agent_intercom::mode::ServerMode;
use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::persistence::db;
use agent_intercom::persistence::session_repo::SessionRepo;
use agent_intercom::slack::commands::dispatch_command;

// ── Test helpers ──────────────────────────────────────────────────────────────

fn make_config(workspace_root: &str, user: &str) -> GlobalConfig {
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 0
ipc_name = "test-command-routing"
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

/// Build an `AppState` with the given `server_mode`.
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

// ── S-T1-021: Malformed arguments → usage message ────────────────────────────

/// S-T1-021 — `steer` with no arguments must respond with a usage error
/// that describes the correct syntax (not a panic or internal error).
#[tokio::test]
async fn steer_without_args_returns_usage_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";

    let state = app_state_with_mode(root, user, ServerMode::Mcp).await;

    let result = dispatch_command("steer", &[], user, "C_TEST", &state).await;

    assert!(
        result.is_err(),
        "steer with no args must return Err (usage error)"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("steer"),
        "error must mention the command name: {err_msg}"
    );
}

/// `task` with no arguments must also return a usage error.
#[tokio::test]
async fn task_without_args_returns_usage_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";

    let state = app_state_with_mode(root, user, ServerMode::Mcp).await;

    let result = dispatch_command("task", &[], user, "C_TEST", &state).await;

    assert!(
        result.is_err(),
        "task with no args must return Err (usage error)"
    );
}

// ── S-T1-022: MCP mode — steer command accepted ───────────────────────────────

/// S-T1-022 — In MCP mode, a valid `steer` command is accepted and
/// the handler returns a confirmation string without error.
#[tokio::test]
async fn mcp_mode_steer_command_accepted() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";

    let state = app_state_with_mode(root, user, ServerMode::Mcp).await;

    // Create an active session so the steer handler can store the instruction.
    {
        let repo = SessionRepo::new(Arc::clone(&state.db));
        let mut session = Session::new(
            user.into(),
            root.into(),
            Some("test".into()),
            SessionMode::Remote,
        );
        session.channel_id = Some("C_TEST".into());
        let created = repo.create(&session).await.expect("create");
        repo.update_status(&created.id, SessionStatus::Active)
            .await
            .expect("activate");
    }

    let result = dispatch_command(
        "steer",
        &["focus", "on", "tests"],
        user,
        "C_TEST",
        &state,
    )
    .await;

    assert!(
        result.is_ok(),
        "steer in MCP mode must return Ok: {result:?}"
    );
}

// ── S-T1-023: ACP-only commands rejected in MCP mode ─────────────────────────

/// S-T1-023 — `session-start` is an ACP-only command. When dispatched in
/// MCP mode, it must return a mode-mismatch message (not an error code),
/// clearly informing the operator that the command requires ACP mode.
#[tokio::test]
async fn session_start_rejected_in_mcp_mode_with_mode_mismatch_message() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";

    let state = app_state_with_mode(root, user, ServerMode::Mcp).await;

    let result = dispatch_command(
        "session-start",
        &["test prompt"],
        user,
        "C_TEST",
        &state,
    )
    .await;

    assert!(
        result.is_ok(),
        "mode-mismatch for ACP command returns Ok with an informational message"
    );
    let msg = result.expect("Ok response");
    assert!(
        msg.contains("ACP") || msg.contains("acp") || msg.contains("mode"),
        "response must mention mode requirement: {msg}"
    );
    assert!(
        msg.contains("session-start"),
        "response must mention the rejected command: {msg}"
    );
}

/// `session-stop` in MCP mode also returns a mode-mismatch message.
#[tokio::test]
async fn session_stop_rejected_in_mcp_mode() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";

    let state = app_state_with_mode(root, user, ServerMode::Mcp).await;

    let result = dispatch_command("session-stop", &[], user, "C_TEST", &state).await;

    assert!(
        result.is_ok(),
        "session-stop in MCP mode must return Ok with mode-mismatch message"
    );
    let msg = result.expect("Ok response");
    assert!(
        msg.contains("session-stop"),
        "response must mention the rejected command: {msg}"
    );
}

/// `session-restart` in MCP mode also returns a mode-mismatch message.
#[tokio::test]
async fn session_restart_rejected_in_mcp_mode() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";

    let state = app_state_with_mode(root, user, ServerMode::Mcp).await;

    let result = dispatch_command("session-restart", &[], user, "C_TEST", &state).await;

    assert!(
        result.is_ok(),
        "session-restart in MCP mode must return Ok with mode-mismatch message"
    );
    let msg = result.expect("Ok response");
    assert!(
        msg.contains("session-restart"),
        "response must mention the rejected command: {msg}"
    );
}

// ── ACP mode — session-start accepted ────────────────────────────────────────

/// `session-start` is accepted in ACP mode and returns a result (success or
/// not-configured error — both are valid outcomes without a running agent).
#[tokio::test]
async fn session_start_accepted_in_acp_mode() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";

    let state = app_state_with_mode(root, user, ServerMode::Acp).await;

    // `session-start <prompt>` is dispatched in ACP mode. The handler may
    // fail because there is no real agent binary ("echo" as host_cli is not
    // a valid agent), but it must NOT return the mode-mismatch message.
    let result = dispatch_command(
        "session-start",
        &["test prompt for acp"],
        user,
        "C_TEST",
        &state,
    )
    .await;

    match &result {
        Ok(msg) => {
            // If it succeeds, it must NOT contain the mode-mismatch text.
            assert!(
                !msg.contains("only available in ACP mode"),
                "session-start in ACP mode must not return mode-mismatch: {msg}"
            );
        }
        Err(err) => {
            // An error is acceptable (e.g., no real agent process) but must
            // not be the mode-mismatch rejection message.
            let err_msg = err.to_string();
            assert!(
                !err_msg.contains("only available in ACP mode"),
                "error must not be a mode-mismatch rejection: {err_msg}"
            );
        }
    }
}

// ── Help command — prefix reflects server mode ────────────────────────────────

/// In MCP mode, the help command output references the `/acom` prefix.
#[tokio::test]
async fn help_command_in_mcp_mode_uses_acom_prefix() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";

    let state = app_state_with_mode(root, user, ServerMode::Mcp).await;

    let result = dispatch_command("help", &[], user, "C_TEST", &state).await;

    assert!(result.is_ok(), "help in MCP mode must return Ok");
    let msg = result.expect("Ok response");
    assert!(
        msg.contains("acom"),
        "MCP mode help must reference 'acom' prefix: {msg}"
    );
    // Must NOT contain the ACP prefix.
    assert!(
        !msg.contains("/arc"),
        "MCP mode help must not reference '/arc' prefix: {msg}"
    );
}

/// In ACP mode, the help command output references the `/arc` prefix.
#[tokio::test]
async fn help_command_in_acp_mode_uses_arc_prefix() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";

    let state = app_state_with_mode(root, user, ServerMode::Acp).await;

    let result = dispatch_command("help", &[], user, "C_TEST", &state).await;

    assert!(result.is_ok(), "help in ACP mode must return Ok");
    let msg = result.expect("Ok response");
    assert!(
        msg.contains("arc"),
        "ACP mode help must reference 'arc' prefix: {msg}"
    );
    // Must NOT contain the MCP prefix.
    assert!(
        !msg.contains("/acom"),
        "ACP mode help must not reference '/acom' prefix: {msg}"
    );
}

/// ACP mode help includes session-start and session-stop commands.
#[tokio::test]
async fn acp_mode_help_includes_session_lifecycle_commands() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";

    let state = app_state_with_mode(root, user, ServerMode::Acp).await;
    let result = dispatch_command("help", &[], user, "C_TEST", &state).await;

    let msg = result.expect("ACP help Ok");
    assert!(
        msg.contains("session-start"),
        "ACP mode help must list session-start: {msg}"
    );
    assert!(
        msg.contains("session-stop"),
        "ACP mode help must list session-stop: {msg}"
    );
}

/// MCP mode help does NOT include ACP-only session lifecycle commands.
#[tokio::test]
async fn mcp_mode_help_omits_acp_only_commands() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";

    let state = app_state_with_mode(root, user, ServerMode::Mcp).await;
    let result = dispatch_command("help", &[], user, "C_TEST", &state).await;

    let msg = result.expect("MCP help Ok");
    // session-start should not appear in MCP mode help (ACP-only gating).
    assert!(
        !msg.contains("session-start"),
        "MCP mode help must omit session-start (ACP-only): {msg}"
    );
}

/// Unknown commands return a descriptive error message, not a panic.
#[tokio::test]
async fn unknown_command_returns_descriptive_message() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST";

    let state = app_state_with_mode(root, user, ServerMode::Mcp).await;

    let result = dispatch_command("definitely-unknown-cmd", &[], user, "C_TEST", &state).await;

    assert!(result.is_ok(), "unknown command must return Ok with error msg");
    let msg = result.expect("Ok response");
    assert!(
        msg.contains("Unknown") || msg.contains("unknown"),
        "response must say 'Unknown command': {msg}"
    );
    assert!(
        msg.contains("definitely-unknown-cmd"),
        "response must name the unknown command: {msg}"
    );
}
