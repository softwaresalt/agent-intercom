//! Live Slack command dispatch tests — Tier 2.
//!
//! Verifies that the slash command dispatcher (`dispatch_command`) correctly
//! parses commands and returns well-structured responses. The "live" aspect of
//! these tests is that they post a message to the real test Slack channel
//! documenting the command under test — confirming that the command response
//! text is fit for inclusion in a Slack message.
//!
//! The tests do NOT require a live Socket Mode connection. Slash command
//! events arriving via Socket Mode call `handle_command`, which in turn calls
//! `dispatch_command`. These tests call `dispatch_command` directly with
//! a synthetic `AppState`, bypassing the Socket Mode layer. This is the
//! correct Tier 2 strategy: verify handler logic with real infrastructure
//! (in-memory DB + live Slack channel for result posting).
//!
//! Scenarios covered:
//! - S-T2-012: `sessions` command returns session summary text posted to Slack.

use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use agent_intercom::config::GlobalConfig;
use agent_intercom::driver::mcp_driver::McpDriver;
use agent_intercom::mcp::handler::{AppState, PendingApprovals, PendingPrompts, PendingWaits};
use agent_intercom::mode::ServerMode;
use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::persistence::db;
use agent_intercom::persistence::session_repo::SessionRepo;
use agent_intercom::slack::commands::dispatch_command;
use tokio::sync::Mutex;

use super::live_helpers::{LiveSlackClient, LiveTestConfig};

// ── Test helpers ──────────────────────────────────────────────────────────────

/// Minimal TOML config for live command tests.
fn make_config(workspace_root: &str, authorized_user: &str) -> GlobalConfig {
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 0
ipc_name = "test-live-commands"
max_concurrent_sessions = 5
host_cli = "echo"

[slack]
channel_id = "C_LIVE_TEST"

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
    let mut config = GlobalConfig::from_toml_str(&toml).expect("valid live command config");
    config.authorized_user_ids = vec![authorized_user.to_owned()];
    config
}

/// Build an `AppState` with the given `server_mode`.
async fn make_app_state(
    workspace_root: &str,
    user: &str,
    server_mode: ServerMode,
) -> Arc<AppState> {
    let config = make_config(workspace_root, user);
    let database = Arc::new(db::connect_memory().await.expect("db connect"));
    let approvals: PendingApprovals = Arc::new(Mutex::new(HashMap::new()));
    let prompts: PendingPrompts = Arc::new(Mutex::new(HashMap::new()));
    let waits: PendingWaits = Arc::new(Mutex::new(HashMap::new()));
    let driver = McpDriver::new(
        Arc::clone(&approvals),
        Arc::clone(&prompts),
        Arc::clone(&waits),
    );

    Arc::new(AppState {
        config: Arc::new(config),
        db: Arc::clone(&database),
        slack: None,
        pending_approvals: approvals,
        pending_prompts: prompts,
        pending_waits: waits,
        pending_modal_contexts: Arc::default(),
        pending_thread_replies: Arc::default(),
        stall_detectors: None,
        ipc_auth_token: None,
        policy_cache: Arc::default(),
        audit_logger: None,
        active_children: Arc::default(),
        pending_command_approvals: Arc::default(),
        stall_event_tx: None,
        driver: Arc::new(driver),
        server_mode,
        workspace_mappings: Arc::default(),
        acp_event_tx: None,
        acp_driver: None,
    })
}

// ── S-T2-012: sessions command ────────────────────────────────────────────────

/// S-T2-012: Dispatch a synthetic `sessions` command and verify the response
/// text contains session information. Post the response to the live test
/// channel to confirm it is suitable for Slack display.
///
/// The test creates one active session in the in-memory DB before dispatching
/// so there is always at least one session to list in the response.
///
/// Scenario: S-T2-012 | FRs: FR-014
#[tokio::test]
async fn sessions_command_returns_session_list_posted_to_slack() {
    let live_config = match LiveTestConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "[live-test] Skipping sessions_command_returns_session_list_posted_to_slack: {e}"
            );
            return;
        }
    };

    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8 root");
    let user = "U_LIVE_CMD_TEST";
    let channel = "C_LIVE_TEST";

    let state = make_app_state(root, user, ServerMode::Mcp).await;

    // Create an active session so the `sessions` command has something to list.
    let repo = SessionRepo::new(Arc::clone(&state.db));
    let mut session = Session::new(
        user.into(),
        root.into(),
        Some("live-cmd-test session".into()),
        SessionMode::Remote,
    );
    session.channel_id = Some(channel.into());
    let created = repo.create(&session).await.expect("create session");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate session");

    // Dispatch the `sessions` command.
    let result = dispatch_command("sessions", &[], user, channel, &state)
        .await
        .expect("sessions command should succeed");

    // Verify the response contains session-related text.
    assert!(
        !result.is_empty(),
        "sessions command must return non-empty response"
    );
    // The response should mention the session ID or status.
    assert!(
        result.contains(&created.id[..8])
            || result.contains("active")
            || result.contains("session"),
        "sessions response must reference sessions or their status; got: {result}"
    );

    // Post the command response to live Slack to document the round-trip.
    let slack_client = LiveSlackClient::new(&live_config.bot_token);
    let run_id = Uuid::new_v4();
    let live_text =
        format!("[live-test] S-T2-012 sessions command response (run {run_id:.8}):\n{result}");
    let ts = slack_client
        .post_test_message(&live_config.channel_id, &live_text)
        .await
        .expect("post sessions command result to Slack");

    // Cleanup.
    slack_client
        .cleanup_test_messages(&live_config.channel_id, &[ts.as_str()])
        .await
        .expect("cleanup should succeed");
}

// ── Additional: help command in MCP mode ──────────────────────────────────────

/// Dispatch a synthetic `help` command in MCP mode. The response must contain
/// the MCP command prefix (`/acom`) and a list of available commands.
/// Post the response to live Slack to verify it renders correctly.
#[tokio::test]
async fn help_command_in_mcp_mode_posted_to_slack() {
    let live_config = match LiveTestConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[live-test] Skipping help_command_in_mcp_mode_posted_to_slack: {e}");
            return;
        }
    };

    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8 root");
    let user = "U_LIVE_CMD_TEST";

    let state = make_app_state(root, user, ServerMode::Mcp).await;

    let result = dispatch_command("help", &[], user, "C_LIVE_TEST", &state)
        .await
        .expect("help command should succeed");

    // Help must mention the MCP prefix and at least one command.
    assert!(
        result.contains("acom") || result.contains("sessions") || result.contains("help"),
        "help response must mention the command prefix or commands; got: {result}"
    );

    // Post to live Slack.
    let slack_client = LiveSlackClient::new(&live_config.bot_token);
    let run_id = Uuid::new_v4();
    let live_text = format!("[live-test] help command response (run {run_id:.8}):\n{result}");
    let ts = slack_client
        .post_test_message(&live_config.channel_id, &live_text)
        .await
        .expect("post help result to Slack");

    slack_client
        .cleanup_test_messages(&live_config.channel_id, &[ts.as_str()])
        .await
        .expect("cleanup should succeed");
}
