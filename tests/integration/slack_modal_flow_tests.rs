//! Integration tests for modal-triggered interaction flows (Phase 2, Task 2.3).
//!
//! Tests the modal open path and `ViewSubmission` dispatch:
//! - S-T1-012: `prompt_refine` with `slack = None` → Ok, oneshot not resolved
//! - S-T1-013: Modal submission (`ViewSubmission`) resolves prompt with instruction text
//!
//! Uses JSON deserialization to construct the complex `SlackInteractionViewSubmissionEvent`
//! type without pulling in the full Slack infrastructure.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use slack_morphism::prelude::{
    SlackActionId, SlackActionType, SlackInteractionActionInfoInit, SlackInteractionViewSubmissionEvent,
    SlackTriggerId,
};
use tokio::sync::{oneshot, Mutex};

use agent_intercom::driver::mcp_driver::McpDriver;
use agent_intercom::mcp::handler::{AppState, PendingApprovals, PendingPrompts, PendingWaits, PromptResponse};
use agent_intercom::mode::ServerMode;
use agent_intercom::models::prompt::{ContinuationPrompt, PromptDecision, PromptType};
use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::persistence::db;
use agent_intercom::persistence::prompt_repo::PromptRepo;
use agent_intercom::persistence::session_repo::SessionRepo;
use agent_intercom::slack::handlers;

// ── Test helpers ──────────────────────────────────────────────────────────────

fn test_config_toml(workspace_root: &str, user: &str) -> agent_intercom::config::GlobalConfig {
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 0
ipc_name = "test-modal-flow"
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
    let mut config = agent_intercom::config::GlobalConfig::from_toml_str(&toml)
        .expect("valid test config");
    config.authorized_user_ids = vec![user.to_owned()];
    config
}

fn make_maps() -> (PendingApprovals, PendingPrompts, PendingWaits) {
    (
        Arc::new(Mutex::new(HashMap::new())),
        Arc::new(Mutex::new(HashMap::new())),
        Arc::new(Mutex::new(HashMap::new())),
    )
}

async fn app_state_with_maps(
    workspace_root: &str,
    user: &str,
    approvals: PendingApprovals,
    prompts: PendingPrompts,
    waits: PendingWaits,
) -> Arc<AppState> {
    let config = test_config_toml(workspace_root, user);
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

/// Create a test session and a pending `ContinuationPrompt` in the DB.
async fn create_session_and_prompt(
    db: &sqlx::SqlitePool,
    user: &str,
    root: &str,
) -> (Session, ContinuationPrompt) {
    let db_arc = Arc::new(db.clone());
    let session_repo = SessionRepo::new(Arc::clone(&db_arc));
    let session = Session::new(
        user.into(),
        root.into(),
        Some("test session".into()),
        SessionMode::Remote,
    );
    let created = session_repo.create(&session).await.expect("create session");
    let active = session_repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate session");

    let prompt_repo = PromptRepo::new(Arc::clone(&db_arc));
    let prompt = ContinuationPrompt::new(
        active.id.clone(),
        "Should the agent continue?".to_owned(),
        PromptType::Continuation,
        None,
        None,
    );
    let created_prompt = prompt_repo.create(&prompt).await.expect("create prompt");

    (active, created_prompt)
}

/// Construct a synthetic `SlackInteractionViewSubmissionEvent` via JSON
/// deserialization, using the block state shape expected by `handle_view_submission`.
fn make_view_submission(
    user_id: &str,
    callback_id: &str,
    instruction_text: &str,
) -> SlackInteractionViewSubmissionEvent {
    let payload = serde_json::json!({
        "team": { "id": "T_TEST", "name": "Test Team" },
        "user": { "id": user_id, "name": "testuser" },
        "view": {
            "id": "V_TEST_001",
            "team_id": "T_TEST",
            "type": "modal",
            "title": { "type": "plain_text", "text": "Test" },
            "blocks": [],
            "hash": "test_hash",
            "callback_id": callback_id,
            "state": {
                "values": {
                    "instruction_block": {
                        "instruction_text": {
                            "type": "plain_text_input",
                            "value": instruction_text
                        }
                    }
                }
            }
        }
    });
    serde_json::from_value(payload).expect("valid view submission payload")
}

/// Synthetic action constructor (matches the one in `slack_interaction_tests.rs`).
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

fn no_trigger() -> SlackTriggerId {
    SlackTriggerId("test-trigger-modal".into())
}

// ── S-T1-012: prompt_refine with slack = None ─────────────────────────────────

/// S-T1-012 — When `prompt_refine` is dispatched with no Slack client,
/// the handler returns `Ok(())` and the oneshot remains unresolved.
///
/// The modal path is skipped, and without a Slack connection there is nothing
/// to open a modal against or register a thread-reply fallback on — the handler
/// gracefully defers and the agent waits.
#[tokio::test]
async fn prompt_refine_with_no_slack_returns_ok_and_does_not_resolve_oneshot() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST_OWNER";

    let (approvals, prompts, waits) = make_maps();
    let state = app_state_with_maps(root, user, approvals, prompts, waits).await;

    let (_, prompt) = create_session_and_prompt(&state.db, user, root).await;
    let prompt_id = prompt.id.clone();

    // Register oneshot — must NOT be resolved when slack = None.
    let (tx, mut rx) = oneshot::channel::<PromptResponse>();
    state
        .pending_prompts
        .lock()
        .await
        .insert(prompt_id.clone(), tx);

    let action = make_action("prompt_refine", &prompt_id);
    let result = handlers::prompt::handle_prompt_action(
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
        "prompt_refine with slack = None must return Ok: {result:?}"
    );

    // The oneshot must NOT be resolved — resolution requires modal/fallback.
    let immediate = rx.try_recv();
    assert!(
        immediate.is_err(),
        "oneshot must not be resolved before modal submission (slack = None)"
    );
}

/// S-T1-012 (variant) — `pending_modal_contexts` must remain empty when
/// `prompt_refine` is dispatched with no channel or message context (both None).
///
/// Without message ts and channel info, no context can be cached.
#[tokio::test]
async fn prompt_refine_with_no_slack_leaves_modal_context_empty() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST_OWNER";

    let (approvals, prompts, waits) = make_maps();
    let state = app_state_with_maps(root, user, approvals, prompts, waits).await;

    let (_, prompt) = create_session_and_prompt(&state.db, user, root).await;
    let prompt_id = prompt.id.clone();

    let (tx, _rx) = oneshot::channel::<PromptResponse>();
    state
        .pending_prompts
        .lock()
        .await
        .insert(prompt_id.clone(), tx);

    let action = make_action("prompt_refine", &prompt_id);
    let _ = handlers::prompt::handle_prompt_action(
        &action,
        user,
        &no_trigger(),
        None,
        None,
        &state,
    )
    .await;

    let ctx = state.pending_modal_contexts.lock().await;
    assert!(
        ctx.is_empty(),
        "pending_modal_contexts must be empty when channel/message are None"
    );
}

// ── S-T1-013: Modal submission resolves prompt ────────────────────────────────

/// S-T1-013 — A synthetic `ViewSubmission` with `callback_id = "prompt_refine:{id}"`
/// resolves the registered oneshot with the instruction text and updates the DB
/// decision to `Refine`.
#[tokio::test]
async fn modal_submission_resolves_prompt_with_instruction_text() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST_OWNER";

    let (approvals, prompts, waits) = make_maps();
    let state = app_state_with_maps(root, user, approvals, prompts, waits).await;

    let (_, prompt) = create_session_and_prompt(&state.db, user, root).await;
    let prompt_id = prompt.id.clone();
    let callback_id = format!("prompt_refine:{prompt_id}");
    let instruction = "Focus on error handling and edge cases";

    // Register oneshot — will be resolved by the ViewSubmission handler.
    let (tx, rx) = oneshot::channel::<PromptResponse>();
    state
        .pending_prompts
        .lock()
        .await
        .insert(prompt_id.clone(), tx);

    // Construct the synthetic view submission event.
    let event = make_view_submission(user, &callback_id, instruction);

    let result = handlers::modal::handle_view_submission(&event, &state).await;
    assert!(
        result.is_ok(),
        "modal submission must return Ok: {result:?}"
    );

    // Oneshot must resolve with the instruction text.
    let response = tokio::time::timeout(Duration::from_millis(500), rx)
        .await
        .expect("oneshot must resolve within timeout")
        .expect("oneshot channel must not be dropped");

    assert_eq!(response.decision, "refine", "decision must be 'refine'");
    assert_eq!(
        response.instruction.as_deref(),
        Some(instruction),
        "instruction must match the modal submission text"
    );

    // DB record must reflect the Refine decision with instruction.
    let repo = PromptRepo::new(Arc::clone(&state.db));
    let updated = repo
        .get_by_id(&prompt_id)
        .await
        .expect("db query")
        .expect("record must exist");
    assert_eq!(
        updated.decision,
        Some(PromptDecision::Refine),
        "DB decision must be Refine"
    );
    assert_eq!(
        updated.instruction.as_deref(),
        Some(instruction),
        "DB instruction must match modal text"
    );
}

/// Modal submission with empty instruction text must return an error.
///
/// The handler rejects empty instructions to prevent the agent from
/// receiving a refine request with no meaningful content.
#[tokio::test]
async fn modal_submission_with_empty_instruction_returns_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST_OWNER";

    let (approvals, prompts, waits) = make_maps();
    let state = app_state_with_maps(root, user, approvals, prompts, waits).await;

    let (_, prompt) = create_session_and_prompt(&state.db, user, root).await;
    let prompt_id = prompt.id.clone();
    let callback_id = format!("prompt_refine:{prompt_id}");

    let event = make_view_submission(user, &callback_id, ""); // ← empty
    let result = handlers::modal::handle_view_submission(&event, &state).await;

    assert!(
        result.is_err(),
        "modal submission with empty instruction must return Err"
    );
}

/// Modal submission from unauthorized user must be rejected.
#[tokio::test]
async fn modal_submission_from_unauthorized_user_returns_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let authorized = "U_AUTHORIZED";
    let intruder = "U_INTRUDER";

    let (approvals, prompts, waits) = make_maps();
    let state = app_state_with_maps(root, authorized, approvals, prompts, waits).await;

    let (_, prompt) = create_session_and_prompt(&state.db, authorized, root).await;
    let callback_id = format!("prompt_refine:{}", prompt.id);

    let event = make_view_submission(intruder, &callback_id, "some instruction");
    let result = handlers::modal::handle_view_submission(&event, &state).await;

    assert!(
        result.is_err(),
        "modal submission from unauthorized user must return Err"
    );
}
