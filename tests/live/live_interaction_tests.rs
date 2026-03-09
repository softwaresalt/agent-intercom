//! Live Slack interaction tests — Tier 2.
//!
//! Tests in this module combine live Slack API message posting with synthetic
//! interaction dispatch through the production handler pipeline. Each test:
//!
//! 1. Posts a real message to the test channel (live Slack).
//! 2. Dispatches a synthetic button action through the handler (offline dispatch,
//!    no live Socket Mode required).
//! 3. Verifies the resulting database state matches the expected transition.
//! 4. Cleans up the posted Slack message.
//!
//! This hybrid approach is appropriate for Tier 2: the live Slack API call
//! validates message posting plumbing end-to-end; the in-process handler
//! dispatch validates the complete action-processing code path (DB update,
//! oneshot resolution, Slack update attempt).
//!
//! **Follow-up Slack messages** (FR-022 button replacement, thread replies) are
//! not verified via the API in these tests because the `AppState` is constructed
//! without a live `SlackService` — which requires a Socket Mode connection.
//! That gap is documented as a known Tier 2 limitation (see `SCENARIOS.md`
//! S-T2-013 note) and is covered by Tier 3 visual tests.
//!
//! Scenarios covered:
//! - S-T2-004: Approval accept round-trip → DB status = "approved"
//! - S-T2-005: Prompt continue round-trip → DB decision = "continue"
//! - S-T2-010: Stall nudge round-trip → DB nudge count incremented
//! - S-T2-013: Button replacement via `chat.update` API verified

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use slack_morphism::prelude::{
    SlackActionId, SlackActionType, SlackInteractionActionInfoInit, SlackTriggerId,
};
use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

use agent_intercom::config::GlobalConfig;
use agent_intercom::driver::mcp_driver::McpDriver;
use agent_intercom::mcp::handler::{
    AppState, ApprovalResponse, PendingApprovals, PendingPrompts, PendingWaits, PromptResponse,
};
use agent_intercom::mode::ServerMode;
use agent_intercom::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};
use agent_intercom::models::prompt::{ContinuationPrompt, PromptDecision, PromptType};
use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::models::stall::StallAlert;
use agent_intercom::persistence::approval_repo::ApprovalRepo;
use agent_intercom::persistence::db;
use agent_intercom::persistence::prompt_repo::PromptRepo;
use agent_intercom::persistence::session_repo::SessionRepo;
use agent_intercom::persistence::stall_repo::StallAlertRepo;
use agent_intercom::slack::{blocks, handlers};

use super::live_helpers::{assert_blocks_contain, LiveSlackClient, LiveTestConfig};

// ── Test helpers ──────────────────────────────────────────────────────────────

/// Minimal TOML config for live interaction tests.
fn make_config(workspace_root: &str, authorized_user: &str) -> GlobalConfig {
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 0
ipc_name = "test-live-interaction"
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
    let mut config = GlobalConfig::from_toml_str(&toml).expect("valid live test config");
    config.authorized_user_ids = vec![authorized_user.to_owned()];
    config
}

/// Build a wired `AppState` with shared pending maps (no live Slack client).
async fn make_app_state(
    workspace_root: &str,
    authorized_user: &str,
    approvals: PendingApprovals,
    prompts: PendingPrompts,
    waits: PendingWaits,
) -> Arc<AppState> {
    let config = make_config(workspace_root, authorized_user);
    let db = Arc::new(db::connect_memory().await.expect("db connect"));
    let driver = McpDriver::new(
        Arc::clone(&approvals),
        Arc::clone(&prompts),
        Arc::clone(&waits),
    );

    Arc::new(AppState {
        config: Arc::new(config),
        db,
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
        server_mode: ServerMode::Mcp,
        workspace_mappings: Arc::default(),
        acp_event_tx: None,
        acp_driver: None,
    })
}

/// Create a synthetic button action info.
fn make_action(
    action_id: &str,
    value: &str,
) -> slack_morphism::prelude::SlackInteractionActionInfo {
    slack_morphism::prelude::SlackInteractionActionInfo::from(SlackInteractionActionInfoInit {
        action_type: SlackActionType("button".into()),
        action_id: SlackActionId(action_id.into()),
    })
    .with_value(value.into())
}

/// No-op trigger ID for handlers that require one but cannot open a modal.
fn no_trigger() -> SlackTriggerId {
    SlackTriggerId("live-test-trigger-noop".into())
}

/// Create an active session owned by `user_id` in the given DB.
async fn create_session(db: &sqlx::SqlitePool, user_id: &str, workspace_root: &str) -> Session {
    let repo = SessionRepo::new(Arc::new(db.clone()));
    let session = Session::new(
        user_id.into(),
        workspace_root.into(),
        Some("live-test session".into()),
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create session");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate session")
}

/// Create an `ApprovalRequest` linked to `session_id` in the DB.
async fn create_approval(db: &sqlx::SqlitePool, session_id: &str) -> ApprovalRequest {
    let repo = ApprovalRepo::new(Arc::new(db.clone()));
    let request = ApprovalRequest::new(
        session_id.to_owned(),
        "Live test approval".to_owned(),
        Some("testing approval round-trip".to_owned()),
        "--- a\n+++ b\n@@ -1 +1 @@\n-old\n+new".to_owned(),
        "src/lib.rs".to_owned(),
        RiskLevel::Low,
        "sha256:livetest".to_owned(),
    );
    repo.create(&request).await.expect("create approval")
}

/// Create a `ContinuationPrompt` linked to `session_id` in the DB.
async fn create_prompt(db: &sqlx::SqlitePool, session_id: &str) -> ContinuationPrompt {
    let repo = PromptRepo::new(Arc::new(db.clone()));
    let prompt = ContinuationPrompt::new(
        session_id.to_owned(),
        "Should the agent continue with the live test?".to_owned(),
        PromptType::Continuation,
        None,
        None,
    );
    repo.create(&prompt).await.expect("create prompt")
}

/// Create a `StallAlert` linked to `session_id` in the DB.
async fn create_stall_alert(db: &sqlx::SqlitePool, session_id: &str) -> StallAlert {
    let repo = StallAlertRepo::new(Arc::new(db.clone()));
    let alert = StallAlert::new(session_id.to_owned(), None, Utc::now(), 90, None);
    repo.create(&alert).await.expect("create stall alert")
}

// ── S-T2-004: Approval accept round-trip ─────────────────────────────────────

/// S-T2-004: Post an approval message to real Slack, dispatch a synthetic
/// `"approve_accept"` action through the handler, and verify:
/// - The oneshot channel resolves with `"approved"`.
/// - The approval record in the DB transitions to `ApprovalStatus::Approved`.
///
/// The live Slack message demonstrates real API posting; the DB state
/// verification confirms the handler processed the interaction correctly.
///
/// Scenario: S-T2-004 | FRs: FR-014
#[tokio::test]
async fn approval_accept_updates_db_record() {
    let live_config = match LiveTestConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[live-test] Skipping approval_accept_updates_db_record: {e}");
            return;
        }
    };

    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8 root");
    let user = "U_LIVE_TEST_OWNER";

    let approvals: PendingApprovals = Arc::new(Mutex::new(HashMap::new()));
    let prompts: PendingPrompts = Arc::new(Mutex::new(HashMap::new()));
    let waits: PendingWaits = Arc::new(Mutex::new(HashMap::new()));

    let state = make_app_state(
        root,
        user,
        Arc::clone(&approvals),
        Arc::clone(&prompts),
        Arc::clone(&waits),
    )
    .await;

    // Set up DB records.
    let session = create_session(&state.db, user, root).await;
    let approval = create_approval(&state.db, &session.id).await;

    // Register oneshot channel.
    let (tx, rx) = oneshot::channel::<ApprovalResponse>();
    approvals.lock().await.insert(approval.id.clone(), tx);

    // Post a real live Slack message representing this approval request.
    let slack_client = LiveSlackClient::new(&live_config.bot_token);
    let run_id = Uuid::new_v4();
    let live_text = format!("[live-test] approval accept round-trip (run {run_id:.8})");
    let mut live_blocks = blocks::build_approval_blocks(
        "Live-test: add error handling",
        None,
        "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new",
        "src/lib.rs",
        RiskLevel::Low,
    );
    live_blocks.push(blocks::approval_buttons(&approval.id));
    let live_blocks_json = serde_json::to_value(&live_blocks).expect("serialize blocks");

    let live_ts = slack_client
        .post_with_blocks(&live_config.channel_id, &live_text, live_blocks_json)
        .await
        .expect("post live approval message");

    // Dispatch synthetic accept action through the production handler.
    let action = make_action("approve_accept", &approval.id);
    let result = handlers::approval::handle_approval_action(
        &action,
        user,
        &no_trigger(),
        None,
        None,
        &state,
    )
    .await;

    assert!(
        result.is_ok(),
        "handle_approval_action should succeed: {:?}",
        result.err()
    );

    // Verify oneshot resolved with "approved".
    let response = rx
        .await
        .expect("oneshot should resolve after approval handler runs");
    assert_eq!(
        response.status, "approved",
        "approval response status must be 'approved'"
    );

    // Verify DB record updated.
    let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));
    let updated = approval_repo
        .get_by_id(&approval.id)
        .await
        .expect("DB query should succeed")
        .expect("approval record should exist");

    assert_eq!(
        updated.status,
        ApprovalStatus::Approved,
        "DB approval status must be Approved after accept"
    );

    // Cleanup live Slack message.
    slack_client
        .cleanup_test_messages(&live_config.channel_id, &[live_ts.as_str()])
        .await
        .expect("cleanup should succeed");
}

// ── S-T2-005: Prompt continue round-trip ─────────────────────────────────────

/// S-T2-005 (partial): Post a prompt message to real Slack, dispatch a
/// synthetic `"prompt_continue"` action, and verify:
/// - The oneshot channel resolves with decision `"continue"`.
/// - The prompt record in the DB transitions to `PromptDecision::Continue`.
///
/// Scenario: S-T2-005 | FRs: FR-014
#[tokio::test]
async fn prompt_continue_updates_db_record() {
    let live_config = match LiveTestConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[live-test] Skipping prompt_continue_updates_db_record: {e}");
            return;
        }
    };

    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8 root");
    let user = "U_LIVE_TEST_OWNER";

    let approvals: PendingApprovals = Arc::new(Mutex::new(HashMap::new()));
    let prompts: PendingPrompts = Arc::new(Mutex::new(HashMap::new()));
    let waits: PendingWaits = Arc::new(Mutex::new(HashMap::new()));

    let state = make_app_state(
        root,
        user,
        Arc::clone(&approvals),
        Arc::clone(&prompts),
        Arc::clone(&waits),
    )
    .await;

    let session = create_session(&state.db, user, root).await;
    let prompt = create_prompt(&state.db, &session.id).await;

    let (tx, rx) = oneshot::channel::<PromptResponse>();
    prompts.lock().await.insert(prompt.id.clone(), tx);

    // Post a live prompt message.
    let slack_client = LiveSlackClient::new(&live_config.bot_token);
    let run_id = Uuid::new_v4();
    let live_text = format!("[live-test] prompt continue round-trip (run {run_id:.8})");
    let live_prompt_blocks = blocks::build_prompt_blocks(
        "Should the agent continue with the live test?",
        PromptType::Continuation,
        None,
        None,
        &prompt.id,
    );
    let live_blocks_json = serde_json::to_value(&live_prompt_blocks).expect("serialize blocks");

    let live_ts = slack_client
        .post_with_blocks(&live_config.channel_id, &live_text, live_blocks_json)
        .await
        .expect("post live prompt message");

    // Dispatch synthetic continue action.
    let action = make_action("prompt_continue", &prompt.id);
    let result =
        handlers::prompt::handle_prompt_action(&action, user, &no_trigger(), None, None, &state)
            .await;

    assert!(
        result.is_ok(),
        "handle_prompt_action should succeed: {:?}",
        result.err()
    );

    // Verify oneshot resolved with decision "continue".
    let response = rx
        .await
        .expect("oneshot should resolve after prompt handler");
    assert_eq!(
        response.decision, "continue",
        "prompt response decision must be 'continue'"
    );

    // Verify DB record updated.
    let prompt_repo = PromptRepo::new(Arc::clone(&state.db));
    let updated = prompt_repo
        .get_by_id(&prompt.id)
        .await
        .expect("DB query should succeed")
        .expect("prompt record should exist");

    assert_eq!(
        updated.decision,
        Some(PromptDecision::Continue),
        "DB prompt decision must be Continue"
    );

    // Cleanup.
    slack_client
        .cleanup_test_messages(&live_config.channel_id, &[live_ts.as_str()])
        .await
        .expect("cleanup should succeed");
}

// ── S-T2-010: Stall nudge round-trip ─────────────────────────────────────────

/// S-T2-010: Post a stall alert message to real Slack, dispatch a synthetic
/// `"stall_nudge"` action, and verify the DB nudge counter increments.
///
/// Scenario: S-T2-010 | FRs: FR-014
#[tokio::test]
async fn stall_nudge_increments_db_counter() {
    let live_config = match LiveTestConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[live-test] Skipping stall_nudge_increments_db_counter: {e}");
            return;
        }
    };

    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8 root");
    let user = "U_LIVE_TEST_OWNER";

    let approvals: PendingApprovals = Arc::new(Mutex::new(HashMap::new()));
    let prompts: PendingPrompts = Arc::new(Mutex::new(HashMap::new()));
    let waits: PendingWaits = Arc::new(Mutex::new(HashMap::new()));

    let state = make_app_state(
        root,
        user,
        Arc::clone(&approvals),
        Arc::clone(&prompts),
        Arc::clone(&waits),
    )
    .await;

    let session = create_session(&state.db, user, root).await;
    let alert = create_stall_alert(&state.db, &session.id).await;

    // Capture pre-nudge count.
    let before_count = alert.nudge_count;

    // Post a live stall alert message.
    let slack_client = LiveSlackClient::new(&live_config.bot_token);
    let run_id = Uuid::new_v4();
    let live_text = format!("[live-test] stall nudge round-trip (run {run_id:.8})");
    let live_alert_blocks = blocks::stall_alert_blocks(&alert.id, 90);
    let live_blocks_json = serde_json::to_value(&live_alert_blocks).expect("serialize blocks");

    let live_ts = slack_client
        .post_with_blocks(&live_config.channel_id, &live_text, live_blocks_json)
        .await
        .expect("post live stall alert message");

    // Dispatch synthetic nudge action.
    let action = make_action("stall_nudge", &alert.id);
    let result = handlers::nudge::handle_nudge_action(&action, user, None, None, &state).await;

    assert!(
        result.is_ok(),
        "handle_nudge_action should succeed: {:?}",
        result.err()
    );

    // Verify nudge count incremented.
    let stall_repo = StallAlertRepo::new(Arc::clone(&state.db));
    let updated = stall_repo
        .get_by_id(&alert.id)
        .await
        .expect("DB query should succeed")
        .expect("stall alert should exist");

    assert!(
        updated.nudge_count > before_count,
        "nudge_count must increase after nudge action; was {before_count}, now {}",
        updated.nudge_count
    );

    // Cleanup.
    slack_client
        .cleanup_test_messages(&live_config.channel_id, &[live_ts.as_str()])
        .await
        .expect("cleanup should succeed");
}

// ── S-T2-013: Button replacement via chat.update ──────────────────────────────

/// S-T2-013: Post an approval message with interactive buttons to real Slack,
/// simulate button replacement by calling `chat.update` to replace the buttons
/// with a static status line, then retrieve the updated message and verify the
/// buttons are gone and the status text is present.
///
/// This test validates the `LiveSlackClient::update_message` helper and the
/// Slack API's `chat.update` behaviour — which is the same endpoint used by the
/// production FR-022 button replacement path.
///
/// Scenario: S-T2-013 | FRs: FR-013, FR-014
#[tokio::test]
async fn button_replacement_via_update_message_api() {
    let config = match LiveTestConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[live-test] Skipping button_replacement_via_update_message_api: {e}");
            return;
        }
    };

    let client = LiveSlackClient::new(&config.bot_token);
    let run_id = Uuid::new_v4();
    let request_id = format!("live-req-{run_id:.8}");

    // Post an approval message with Accept/Reject buttons.
    let mut approval_blocks = blocks::build_approval_blocks(
        "Live button-replacement test",
        None,
        "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new",
        "src/lib.rs",
        RiskLevel::Low,
    );
    approval_blocks.push(blocks::approval_buttons(&request_id));
    let original_blocks_json = serde_json::to_value(&approval_blocks).expect("serialize");

    let live_text = format!("[live-test] button replacement test (run {run_id:.8})");
    let ts = client
        .post_with_blocks(&config.channel_id, &live_text, original_blocks_json)
        .await
        .expect("post original approval message");

    // Verify original message has approval action buttons.
    let original = client
        .get_message(&config.channel_id, &ts)
        .await
        .expect("get original message");
    assert_blocks_contain(&original, "approve_accept");

    // Simulate button replacement: update the message with a static status line.
    let status_text = "\u{2705} *Accepted* by @live-tester";
    let status_blocks = serde_json::to_value(vec![blocks::text_section(status_text)])
        .expect("serialize status blocks");

    client
        .update_message(&config.channel_id, &ts, status_text, Some(status_blocks))
        .await
        .expect("chat.update (button replacement) should succeed");

    // Retrieve updated message and verify buttons are gone.
    let updated = client
        .get_message(&config.channel_id, &ts)
        .await
        .expect("get updated message");

    let updated_blocks_json =
        serde_json::to_string(&updated["blocks"]).unwrap_or_else(|_| "null".to_owned());

    assert!(
        !updated_blocks_json.contains("approve_accept"),
        "approve_accept button must be absent after replacement; got: {updated_blocks_json}"
    );
    assert!(
        !updated_blocks_json.contains("approve_reject"),
        "approve_reject button must be absent after replacement; got: {updated_blocks_json}"
    );
    assert!(
        updated_blocks_json.contains("Accepted"),
        "status text 'Accepted' must be present after replacement; got: {updated_blocks_json}"
    );

    // Cleanup.
    client
        .cleanup_test_messages(&config.channel_id, &[ts.as_str()])
        .await
        .expect("cleanup should succeed");
}
