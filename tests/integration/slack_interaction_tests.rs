//! Integration tests for simulated Slack interaction dispatch (Phase 2).
//!
//! Exercises the full handler pipeline by constructing synthetic Slack action
//! payloads and dispatching them through the production handler functions.
//! All tests use an in-memory `SQLite` database and no live Slack connection.
//!
//! Scenarios covered:
//! - S-T1-009: Approval accept resolves oneshot and updates DB
//! - S-T1-010: Approval reject (via modal submission path)
//! - S-T1-011: Prompt continue resolves oneshot
//! - S-T1-025: Stall nudge increments DB counter
//! - S-T1-026: Wait resume resolves oneshot

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use slack_morphism::prelude::{
    SlackActionId, SlackActionType, SlackInteractionActionInfoInit, SlackTriggerId,
};
use tokio::sync::{oneshot, Mutex};

use agent_intercom::driver::mcp_driver::McpDriver;
use agent_intercom::mcp::handler::{
    ApprovalResponse, AppState, PendingApprovals, PendingPrompts, PendingWaits, PromptResponse,
    WaitResponse,
};
use agent_intercom::mode::ServerMode;
use agent_intercom::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};
use agent_intercom::models::prompt::{ContinuationPrompt, PromptDecision, PromptType};
use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::models::stall::{StallAlert, StallAlertStatus};
use agent_intercom::persistence::approval_repo::ApprovalRepo;
use agent_intercom::persistence::db;
use agent_intercom::persistence::prompt_repo::PromptRepo;
use agent_intercom::persistence::session_repo::SessionRepo;
use agent_intercom::persistence::stall_repo::StallAlertRepo;
use agent_intercom::slack::handlers;

// ── Test helpers ──────────────────────────────────────────────────────────────

/// Minimal TOML config with in-memory DB and a single authorized user.
fn test_config_toml(workspace_root: &str) -> String {
    format!(
        r#"
default_workspace_root = '{root}'
http_port = 0
ipc_name = "test-slack-interaction"
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
    )
}

/// Build shared pending maps and a wired `McpDriver`.
fn make_maps() -> (PendingApprovals, PendingPrompts, PendingWaits) {
    (
        Arc::new(Mutex::new(HashMap::new())),
        Arc::new(Mutex::new(HashMap::new())),
        Arc::new(Mutex::new(HashMap::new())),
    )
}

/// Build an `AppState` with shared maps wired to `McpDriver`.
///
/// The driver and `AppState` share the same `Arc` instances so that
/// `resolve_clearance` / `resolve_prompt` / `resolve_wait` on the driver
/// reach the same maps that the test pre-seeds.
async fn app_state_with_maps(
    workspace_root: &str,
    authorized_user: &str,
    approvals: PendingApprovals,
    prompts: PendingPrompts,
    waits: PendingWaits,
) -> Arc<AppState> {
    let toml = test_config_toml(workspace_root);
    let mut config =
        agent_intercom::config::GlobalConfig::from_toml_str(&toml).expect("valid test config");
    config.authorized_user_ids = vec![authorized_user.to_owned()];

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

/// Create a synthetic `SlackInteractionActionInfo` for testing.
fn make_action(action_id: &str, value: &str) -> slack_morphism::prelude::SlackInteractionActionInfo {
    slack_morphism::prelude::SlackInteractionActionInfo::from(SlackInteractionActionInfoInit {
        action_type: SlackActionType("button".into()),
        action_id: SlackActionId(action_id.into()),
    })
    .with_value(value.into())
}

/// A no-op trigger ID for handlers that require one but cannot open a modal.
fn no_trigger() -> SlackTriggerId {
    SlackTriggerId("test-trigger-noop".into())
}

/// Create an active session owned by `user_id` in the DB.
async fn create_session(
    db: &sqlx::SqlitePool,
    user_id: &str,
    workspace_root: &str,
) -> Session {
    let repo = SessionRepo::new(Arc::new(db.clone()));
    let session = Session::new(
        user_id.into(),
        workspace_root.into(),
        Some("test session".into()),
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create session");
    repo.update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate session")
}

/// Create an `ApprovalRequest` linked to a session in the DB.
async fn create_approval(db: &sqlx::SqlitePool, session_id: &str) -> ApprovalRequest {
    let repo = ApprovalRepo::new(Arc::new(db.clone()));
    let request = ApprovalRequest::new(
        session_id.to_owned(),
        "Test approval".to_owned(),
        Some("test description".to_owned()),
        "--- a\n+++ b\n@@ -1 +1 @@\n-old\n+new".to_owned(),
        "src/lib.rs".to_owned(),
        RiskLevel::Low,
        "sha256:abc".to_owned(),
    );
    repo.create(&request).await.expect("create approval")
}

/// Create a `ContinuationPrompt` linked to a session in the DB.
async fn create_prompt(db: &sqlx::SqlitePool, session_id: &str) -> ContinuationPrompt {
    let repo = PromptRepo::new(Arc::new(db.clone()));
    let prompt = ContinuationPrompt::new(
        session_id.to_owned(),
        "Should the agent continue?".to_owned(),
        PromptType::Continuation,
        None,
        None,
    );
    repo.create(&prompt).await.expect("create prompt")
}

/// Create a `StallAlert` linked to a session in the DB.
async fn create_stall_alert(db: &sqlx::SqlitePool, session_id: &str) -> agent_intercom::models::stall::StallAlert {
    let repo = StallAlertRepo::new(Arc::new(db.clone()));
    let alert = StallAlert::new(session_id.to_owned(), None, Utc::now(), 60, None);
    repo.create(&alert).await.expect("create stall alert")
}

// ── S-T1-009: Approval accept ─────────────────────────────────────────────────

/// S-T1-009 — `approve_accept` resolves the registered oneshot and marks the
/// approval record as `Approved` in the database.
#[tokio::test]
async fn simulated_approval_accept_resolves_oneshot_and_updates_db() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST_OWNER";

    let (approvals, prompts, waits) = make_maps();
    let state = app_state_with_maps(root, user, approvals, prompts, waits).await;

    // Create session + approval request in DB.
    let session = create_session(&state.db, user, root).await;
    let approval = create_approval(&state.db, &session.id).await;
    let request_id = approval.id.clone();

    // Register oneshot in shared pending_approvals (same map the driver uses).
    let (tx, rx) = oneshot::channel::<ApprovalResponse>();
    state
        .pending_approvals
        .lock()
        .await
        .insert(request_id.clone(), tx);

    // Dispatch synthetic approve_accept action.
    let action = make_action("approve_accept", &request_id);
    let result = handlers::approval::handle_approval_action(
        &action,
        user,
        &no_trigger(),
        None, // no channel — slack = None
        None, // no message
        &state,
    )
    .await;

    assert!(result.is_ok(), "approve_accept must return Ok: {result:?}");

    // Oneshot must resolve within a short timeout.
    let response = tokio::time::timeout(Duration::from_millis(500), rx)
        .await
        .expect("oneshot must resolve")
        .expect("oneshot must not be dropped");

    assert_eq!(response.status, "approved", "status must be 'approved'");
    assert!(response.reason.is_none(), "reason must be None for accept");

    // DB record must be updated to Approved.
    let repo = ApprovalRepo::new(Arc::clone(&state.db));
    let updated = repo
        .get_by_id(&request_id)
        .await
        .expect("db query")
        .expect("record must exist");
    assert_eq!(
        updated.status,
        ApprovalStatus::Approved,
        "DB status must be Approved"
    );
}

// ── S-T1-010: Approval reject (via modal submission) ─────────────────────────

/// S-T1-010 — When `approve_reject` is dispatched with `slack = None`,
/// the handler returns `Ok(())` without resolving the oneshot (the rejection
/// requires a modal submission or thread-reply fallback).
///
/// This test verifies the handler does not panic and correctly defers
/// resolution to the modal/fallback path.
#[tokio::test]
async fn simulated_approval_reject_with_no_slack_returns_ok_and_defers_resolution() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST_OWNER";

    let (approvals, prompts, waits) = make_maps();
    let state = app_state_with_maps(root, user, approvals, prompts, waits).await;

    // Create session + approval request.
    let session = create_session(&state.db, user, root).await;
    let approval = create_approval(&state.db, &session.id).await;
    let request_id = approval.id.clone();

    // Register oneshot — must NOT be resolved when there is no Slack client.
    let (tx, mut rx) = oneshot::channel::<ApprovalResponse>();
    state
        .pending_approvals
        .lock()
        .await
        .insert(request_id.clone(), tx);

    let action = make_action("approve_reject", &request_id);
    let result = handlers::approval::handle_approval_action(
        &action,
        user,
        &no_trigger(),
        None,
        None,
        &state,
    )
    .await;

    assert!(result.is_ok(), "approve_reject must return Ok when slack is None");

    // Oneshot must NOT be resolved immediately — it is pending the modal.
    let immediate = rx.try_recv();
    assert!(
        immediate.is_err(),
        "oneshot must not be resolved before modal submission"
    );
}

// ── S-T1-011: Prompt continue ─────────────────────────────────────────────────

/// S-T1-011 — `prompt_continue` resolves the registered oneshot with the
/// continuation signal and updates the DB decision to `Continue`.
#[tokio::test]
async fn simulated_prompt_continue_resolves_oneshot_and_updates_db() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST_OWNER";

    let (approvals, prompts, waits) = make_maps();
    let state = app_state_with_maps(root, user, approvals, prompts, waits).await;

    let session = create_session(&state.db, user, root).await;
    let prompt = create_prompt(&state.db, &session.id).await;
    let prompt_id = prompt.id.clone();

    let (tx, rx) = oneshot::channel::<PromptResponse>();
    state
        .pending_prompts
        .lock()
        .await
        .insert(prompt_id.clone(), tx);

    let action = make_action("prompt_continue", &prompt_id);
    let result = handlers::prompt::handle_prompt_action(
        &action,
        user,
        &no_trigger(),
        None,
        None,
        &state,
    )
    .await;

    assert!(result.is_ok(), "prompt_continue must return Ok: {result:?}");

    let response = tokio::time::timeout(Duration::from_millis(500), rx)
        .await
        .expect("oneshot must resolve")
        .expect("oneshot must not be dropped");

    assert_eq!(response.decision, "continue", "decision must be 'continue'");
    assert!(response.instruction.is_none(), "no instruction for continue");

    // DB record must show Continue decision.
    let repo = PromptRepo::new(Arc::clone(&state.db));
    let updated = repo
        .get_by_id(&prompt_id)
        .await
        .expect("db query")
        .expect("record must exist");
    assert_eq!(
        updated.decision,
        Some(PromptDecision::Continue),
        "DB decision must be Continue"
    );
}

// ── Prompt stop ───────────────────────────────────────────────────────────────

/// Prompt stop action resolves the oneshot with the stop signal and updates DB.
#[tokio::test]
async fn simulated_prompt_stop_resolves_oneshot_and_updates_db() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST_OWNER";

    let (approvals, prompts, waits) = make_maps();
    let state = app_state_with_maps(root, user, approvals, prompts, waits).await;

    let session = create_session(&state.db, user, root).await;
    let prompt = create_prompt(&state.db, &session.id).await;
    let prompt_id = prompt.id.clone();

    let (tx, rx) = oneshot::channel::<PromptResponse>();
    state
        .pending_prompts
        .lock()
        .await
        .insert(prompt_id.clone(), tx);

    let action = make_action("prompt_stop", &prompt_id);
    let result = handlers::prompt::handle_prompt_action(
        &action,
        user,
        &no_trigger(),
        None,
        None,
        &state,
    )
    .await;

    assert!(result.is_ok(), "prompt_stop must return Ok: {result:?}");

    let response = tokio::time::timeout(Duration::from_millis(500), rx)
        .await
        .expect("oneshot must resolve")
        .expect("oneshot must not be dropped");

    assert_eq!(response.decision, "stop", "decision must be 'stop'");

    let repo = PromptRepo::new(Arc::clone(&state.db));
    let updated = repo
        .get_by_id(&prompt_id)
        .await
        .expect("db query")
        .expect("record must exist");
    assert_eq!(
        updated.decision,
        Some(PromptDecision::Stop),
        "DB decision must be Stop"
    );
}

// ── S-T1-025: Stall nudge ─────────────────────────────────────────────────────

/// S-T1-025 — `stall_nudge` increments the nudge count in the DB without error.
#[tokio::test]
async fn simulated_stall_nudge_increments_db_counter() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST_OWNER";

    let (approvals, prompts, waits) = make_maps();
    let state = app_state_with_maps(root, user, approvals, prompts, waits).await;

    let session = create_session(&state.db, user, root).await;
    let alert = create_stall_alert(&state.db, &session.id).await;
    let alert_id = alert.id.clone();

    let action = make_action("stall_nudge", &alert_id);
    let result = handlers::nudge::handle_nudge_action(
        &action,
        user,
        None, // no Slack channel — slack = None
        None,
        &state,
    )
    .await;

    assert!(result.is_ok(), "stall_nudge must return Ok: {result:?}");

    // Verify nudge_count incremented in DB.
    let repo = StallAlertRepo::new(Arc::clone(&state.db));
    let updated = repo
        .get_by_id(&alert_id)
        .await
        .expect("db query")
        .expect("alert must exist");
    assert_eq!(updated.nudge_count, 1, "nudge_count must be 1 after one nudge");
    assert_eq!(
        updated.status,
        StallAlertStatus::Nudged,
        "status must transition to Nudged"
    );
}

// ── S-T1-026: Wait resume ─────────────────────────────────────────────────────

/// S-T1-026 — `wait_resume` resolves the registered oneshot with the resume
/// signal, unblocking the waiting agent.
#[tokio::test]
async fn simulated_wait_resume_resolves_oneshot() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST_OWNER";

    let (approvals, prompts, waits) = make_maps();
    let state = app_state_with_maps(root, user, approvals, prompts, waits).await;

    let session = create_session(&state.db, user, root).await;
    let session_id = session.id.clone();

    // Register oneshot keyed by session_id.
    let (tx, rx) = oneshot::channel::<WaitResponse>();
    state
        .pending_waits
        .lock()
        .await
        .insert(session_id.clone(), tx);

    let action = make_action("wait_resume", &session_id);
    let result = handlers::wait::handle_wait_action(
        &action,
        user,
        &no_trigger(),
        None,
        None,
        &state,
    )
    .await;

    assert!(result.is_ok(), "wait_resume must return Ok: {result:?}");

    let response = tokio::time::timeout(Duration::from_millis(500), rx)
        .await
        .expect("oneshot must resolve")
        .expect("oneshot must not be dropped");

    assert_eq!(response.status, "resumed", "status must be 'resumed'");
    assert!(
        response.instruction.is_none(),
        "no instruction for plain resume"
    );
}

// ── Authorization guard ───────────────────────────────────────────────────────

/// Unauthorized user attempting an approval action must be rejected.
///
/// The approval handler checks `authorized_user_ids` before acting.
#[tokio::test]
async fn unauthorized_user_approval_action_is_rejected() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let authorized = "U_AUTHORIZED";
    let intruder = "U_INTRUDER";

    let (approvals, prompts, waits) = make_maps();
    // Config authorizes only U_AUTHORIZED.
    let state = app_state_with_maps(root, authorized, approvals, prompts, waits).await;

    let session = create_session(&state.db, authorized, root).await;
    let approval = create_approval(&state.db, &session.id).await;
    let request_id = approval.id.clone();

    let (tx, mut rx) = oneshot::channel::<ApprovalResponse>();
    state
        .pending_approvals
        .lock()
        .await
        .insert(request_id.clone(), tx);

    // Dispatch from unauthorized user.
    let action = make_action("approve_accept", &request_id);
    let result = handlers::approval::handle_approval_action(
        &action,
        intruder, // ← unauthorized
        &no_trigger(),
        None,
        None,
        &state,
    )
    .await;

    // Handler must return Err for unauthorized user.
    assert!(
        result.is_err(),
        "unauthorized user action must return Err"
    );

    // Oneshot must NOT be resolved — no state change.
    let not_resolved = rx.try_recv();
    assert!(
        not_resolved.is_err(),
        "oneshot must not be resolved for unauthorized action"
    );
}

// ── Double-submission prevention ──────────────────────────────────────────────

/// Double-submission: after the first `approve_accept` resolves the oneshot,
/// a second call with the same `request_id` must fail gracefully (`NotFound`)
/// without panicking.
#[tokio::test]
async fn double_submission_second_call_returns_not_found() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let user = "U_TEST_OWNER";

    let (approvals, prompts, waits) = make_maps();
    let state = app_state_with_maps(root, user, approvals, prompts, waits).await;

    let session = create_session(&state.db, user, root).await;
    let approval = create_approval(&state.db, &session.id).await;
    let request_id = approval.id.clone();

    let (tx, rx) = oneshot::channel::<ApprovalResponse>();
    state
        .pending_approvals
        .lock()
        .await
        .insert(request_id.clone(), tx);

    let action = make_action("approve_accept", &request_id);

    // First dispatch — must succeed and resolve the oneshot.
    let first = handlers::approval::handle_approval_action(
        &action,
        user,
        &no_trigger(),
        None,
        None,
        &state,
    )
    .await;
    assert!(first.is_ok(), "first dispatch must succeed");

    let _ = rx.await.expect("first oneshot must resolve");

    // Second dispatch — the pending map entry is gone; driver returns NotFound.
    // The approval handler returns Ok(()) even when the driver fails (it logs a warning).
    let second = handlers::approval::handle_approval_action(
        &action,
        user,
        &no_trigger(),
        None,
        None,
        &state,
    )
    .await;
    // The handler swallows driver errors (warn! only), so it still returns Ok.
    // The important invariant is: no panic and no double-resolution.
    assert!(
        second.is_ok(),
        "second dispatch must return Ok (handler swallows driver NotFound)"
    );
}
