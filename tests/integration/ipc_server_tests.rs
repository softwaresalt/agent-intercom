//! Integration tests for IPC server command dispatch and authentication.
//!
//! Validates:
//! - S053: Valid auth token accepted
//! - S054: Invalid auth token rejected
//! - S055: Missing auth token rejected
//! - S057: `list` command returns active sessions
//! - S059: `approve` resolves pending approval via oneshot
//! - S060: `reject` resolves with reason via oneshot
//! - S062: `resume` resolves pending wait via oneshot
//! - S064: `mode` command changes session operational mode
//!
//! FR-008 — IPC Server Command Dispatch

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use agent_intercom::config::GlobalConfig;
use agent_intercom::ipc::server::spawn_ipc_server;
use agent_intercom::mcp::handler::{AppState, ApprovalResponse, WaitResponse};
use agent_intercom::models::approval::{ApprovalRequest, RiskLevel};
use agent_intercom::models::session::{SessionMode, SessionStatus};
use agent_intercom::persistence::approval_repo::ApprovalRepo;
use agent_intercom::persistence::db;
use agent_intercom::persistence::session_repo::SessionRepo;
use interprocess::local_socket::{
    traits::Stream as SyncStreamTrait, GenericNamespaced, Stream, ToNsName,
};
use sqlx::SqlitePool;
use tokio::sync::{oneshot, Mutex};
use tokio_util::sync::CancellationToken;

use super::test_helpers::create_active_session;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Generate a unique IPC socket name for test isolation.
fn unique_ipc_name() -> String {
    format!("ti{}", uuid::Uuid::new_v4().simple())
}

/// Build a `GlobalConfig` with the given IPC socket name.
fn ipc_test_config(workspace_root: &str, ipc_name: &str) -> GlobalConfig {
    let toml = format!(
        r#"
default_workspace_root = '{root}'
http_port = 0
ipc_name = "{ipc}"
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
        ipc = ipc_name,
    );
    GlobalConfig::from_toml_str(&toml).expect("valid ipc test config")
}

/// Build an `AppState` with in-memory `SQLite`, configurable IPC auth token.
fn ipc_app_state(
    db: Arc<SqlitePool>,
    workspace_root: &str,
    ipc_name: &str,
    auth_token: Option<String>,
) -> Arc<AppState> {
    let config = ipc_test_config(workspace_root, ipc_name);
    Arc::new(AppState {
        config: Arc::new(config),
        db,
        slack: None,
        pending_approvals: Arc::new(Mutex::new(HashMap::new())),
        pending_prompts: Arc::new(Mutex::new(HashMap::new())),
        pending_waits: Arc::new(Mutex::new(HashMap::new())),
        pending_modal_contexts: Default::default(),
        stall_detectors: None,
        ipc_auth_token: auth_token,
    })
}

/// Send a single IPC JSON command via the blocking socket API and return the parsed response.
///
/// Retries connection for up to 1 second to allow the server startup time.
async fn send_ipc(ipc_name: String, request: serde_json::Value) -> serde_json::Value {
    tokio::task::spawn_blocking(move || {
        let ns_name = ipc_name
            .to_ns_name::<GenericNamespaced>()
            .expect("valid ipc name");

        // Retry connection until the server is ready.
        let mut last_err = String::new();
        for _ in 0..20 {
            match Stream::connect(ns_name.clone()) {
                Ok(stream) => {
                    use std::io::{BufRead, BufReader, Write};
                    let mut stream = stream;
                    let mut req = serde_json::to_string(&request).expect("serialize request");
                    req.push('\n');
                    stream.write_all(req.as_bytes()).expect("write request");
                    stream.flush().expect("flush");

                    let mut reader = BufReader::new(&stream);
                    let mut line = String::new();
                    reader.read_line(&mut line).expect("read response");
                    return serde_json::from_str(line.trim()).expect("parse response");
                }
                Err(err) => {
                    last_err = err.to_string();
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
        panic!("IPC server did not become ready: {last_err}");
    })
    .await
    .expect("spawn_blocking")
}

// ── S053: valid auth token accepted ──────────────────────────────────────────

#[tokio::test]
async fn ipc_valid_auth_token_accepted() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let ipc_name = unique_ipc_name();
    let db = Arc::new(db::connect_memory().await.expect("db connect"));
    let state = ipc_app_state(db, root, &ipc_name, Some("secret-token".into()));
    let ct = CancellationToken::new();
    spawn_ipc_server(Arc::clone(&state), ct.clone()).expect("spawn ipc server");

    let resp = send_ipc(
        ipc_name,
        serde_json::json!({"command": "list", "auth_token": "secret-token"}),
    )
    .await;

    ct.cancel();
    assert!(
        resp["ok"].as_bool().unwrap_or(false),
        "valid token should be accepted: {resp}"
    );
}

// ── S054: invalid auth token rejected ────────────────────────────────────────

#[tokio::test]
async fn ipc_invalid_auth_token_rejected() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let ipc_name = unique_ipc_name();
    let db = Arc::new(db::connect_memory().await.expect("db connect"));
    let state = ipc_app_state(db, root, &ipc_name, Some("secret-token".into()));
    let ct = CancellationToken::new();
    spawn_ipc_server(Arc::clone(&state), ct.clone()).expect("spawn ipc server");

    let resp = send_ipc(
        ipc_name,
        serde_json::json!({"command": "list", "auth_token": "wrong-token"}),
    )
    .await;

    ct.cancel();
    assert!(
        !resp["ok"].as_bool().unwrap_or(true),
        "invalid token should be rejected: {resp}"
    );
    assert_eq!(
        resp["error"], "unauthorized",
        "error should be 'unauthorized'"
    );
}

// ── S055: missing auth token rejected ────────────────────────────────────────

#[tokio::test]
async fn ipc_missing_auth_token_rejected() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let ipc_name = unique_ipc_name();
    let db = Arc::new(db::connect_memory().await.expect("db connect"));
    let state = ipc_app_state(db, root, &ipc_name, Some("secret-token".into()));
    let ct = CancellationToken::new();
    spawn_ipc_server(Arc::clone(&state), ct.clone()).expect("spawn ipc server");

    // No auth_token field in the request.
    let resp = send_ipc(ipc_name, serde_json::json!({"command": "list"})).await;

    ct.cancel();
    assert!(
        !resp["ok"].as_bool().unwrap_or(true),
        "missing token should be rejected: {resp}"
    );
    assert_eq!(
        resp["error"], "unauthorized",
        "error should be 'unauthorized'"
    );
}

// ── S057: list returns active sessions ───────────────────────────────────────

#[tokio::test]
async fn ipc_list_returns_active_sessions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let ipc_name = unique_ipc_name();
    let db = Arc::new(db::connect_memory().await.expect("db connect"));

    // Create 2 active sessions and 1 terminated session.
    let repo = SessionRepo::new(Arc::clone(&db));
    create_active_session(&db, root).await;
    create_active_session(&db, root).await;
    // Terminated session: create then mark terminated.
    let s = create_active_session(&db, root).await;
    repo.update_status(&s.id, SessionStatus::Terminated)
        .await
        .expect("terminate session");

    let state = ipc_app_state(Arc::clone(&db), root, &ipc_name, None);
    let ct = CancellationToken::new();
    spawn_ipc_server(Arc::clone(&state), ct.clone()).expect("spawn ipc server");

    let resp = send_ipc(ipc_name, serde_json::json!({"command": "list"})).await;

    ct.cancel();
    assert!(
        resp["ok"].as_bool().unwrap_or(false),
        "list should succeed: {resp}"
    );
    let sessions = resp["data"]["sessions"].as_array().expect("sessions array");
    assert_eq!(
        sessions.len(),
        2,
        "list should return only 2 active sessions"
    );
}

// ── S059: approve resolves pending approval oneshot ───────────────────────────

#[tokio::test]
async fn ipc_approve_resolves_pending_approval() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let ipc_name = unique_ipc_name();
    let db = Arc::new(db::connect_memory().await.expect("db connect"));

    // Create the session and approval request in DB.
    let session = create_active_session(&db, root).await;
    let approval = ApprovalRequest::new(
        session.id.clone(),
        "test proposal".into(),
        None,
        "diff content".into(),
        "src/lib.rs".into(),
        RiskLevel::Low,
        "abc123".into(),
    );
    let approval = ApprovalRepo::new(Arc::clone(&db))
        .create(&approval)
        .await
        .expect("create approval");

    // Wire up the oneshot sender in pending_approvals.
    let (tx, rx) = oneshot::channel::<ApprovalResponse>();
    let state = ipc_app_state(Arc::clone(&db), root, &ipc_name, None);
    {
        let mut pending = state.pending_approvals.lock().await;
        pending.insert(approval.id.clone(), tx);
    }

    let ct = CancellationToken::new();
    spawn_ipc_server(Arc::clone(&state), ct.clone()).expect("spawn ipc server");

    let resp = send_ipc(
        ipc_name,
        serde_json::json!({"command": "approve", "id": approval.id}),
    )
    .await;
    ct.cancel();

    assert!(
        resp["ok"].as_bool().unwrap_or(false),
        "approve should succeed: {resp}"
    );

    // Verify the oneshot was fired with approved status.
    let approval_resp = tokio::time::timeout(Duration::from_secs(1), rx)
        .await
        .expect("oneshot should resolve within 1 s")
        .expect("channel should not be dropped");
    assert_eq!(approval_resp.status, "approved");
    assert!(approval_resp.reason.is_none());
}

// ── S060: reject resolves oneshot with reason ─────────────────────────────────

#[tokio::test]
async fn ipc_reject_resolves_with_reason() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let ipc_name = unique_ipc_name();
    let db = Arc::new(db::connect_memory().await.expect("db connect"));

    // Create the session and approval request in DB.
    let session = create_active_session(&db, root).await;
    let approval = ApprovalRequest::new(
        session.id.clone(),
        "test proposal".into(),
        None,
        "diff content".into(),
        "src/lib.rs".into(),
        RiskLevel::High,
        "abc123".into(),
    );
    let approval = ApprovalRepo::new(Arc::clone(&db))
        .create(&approval)
        .await
        .expect("create approval");

    // Wire up the oneshot sender.
    let (tx, rx) = oneshot::channel::<ApprovalResponse>();
    let state = ipc_app_state(Arc::clone(&db), root, &ipc_name, None);
    {
        let mut pending = state.pending_approvals.lock().await;
        pending.insert(approval.id.clone(), tx);
    }

    let ct = CancellationToken::new();
    spawn_ipc_server(Arc::clone(&state), ct.clone()).expect("spawn ipc server");

    let resp = send_ipc(
        ipc_name,
        serde_json::json!({
            "command": "reject",
            "id": approval.id,
            "reason": "too dangerous"
        }),
    )
    .await;
    ct.cancel();

    assert!(
        resp["ok"].as_bool().unwrap_or(false),
        "reject should succeed: {resp}"
    );

    // Verify the oneshot was fired with rejected status and reason.
    let approval_resp = tokio::time::timeout(Duration::from_secs(1), rx)
        .await
        .expect("oneshot should resolve within 1 s")
        .expect("channel should not be dropped");
    assert_eq!(approval_resp.status, "rejected");
    assert_eq!(approval_resp.reason.as_deref(), Some("too dangerous"));
}

// ── S062: resume resolves pending wait oneshot ────────────────────────────────

#[tokio::test]
async fn ipc_resume_resolves_pending_wait() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let ipc_name = unique_ipc_name();
    let db = Arc::new(db::connect_memory().await.expect("db connect"));

    // Create an active session.
    let session = create_active_session(&db, root).await;

    // Wire up the pending wait oneshot.
    let (tx, rx) = oneshot::channel::<WaitResponse>();
    let state = ipc_app_state(Arc::clone(&db), root, &ipc_name, None);
    {
        let mut pending = state.pending_waits.lock().await;
        pending.insert(session.id.clone(), tx);
    }

    let ct = CancellationToken::new();
    spawn_ipc_server(Arc::clone(&state), ct.clone()).expect("spawn ipc server");

    let resp = send_ipc(
        ipc_name,
        serde_json::json!({
            "command": "resume",
            "id": session.id,
            "instruction": "deploy to staging"
        }),
    )
    .await;
    ct.cancel();

    assert!(
        resp["ok"].as_bool().unwrap_or(false),
        "resume should succeed: {resp}"
    );

    // Verify the oneshot was fired with resumed status.
    let wait_resp = tokio::time::timeout(Duration::from_secs(1), rx)
        .await
        .expect("oneshot should resolve within 1 s")
        .expect("channel should not be dropped");
    assert_eq!(wait_resp.status, "resumed");
    assert_eq!(wait_resp.instruction.as_deref(), Some("deploy to staging"));
}

// ── S064: mode command changes session operational mode ───────────────────────

#[tokio::test]
async fn ipc_mode_changes_session_mode() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_str().expect("utf8");
    let ipc_name = unique_ipc_name();
    let db = Arc::new(db::connect_memory().await.expect("db connect"));

    // Create an active session (default Remote mode).
    let session = create_active_session(&db, root).await;
    assert_eq!(
        session.mode,
        SessionMode::Remote,
        "initial mode should be Remote"
    );

    let state = ipc_app_state(Arc::clone(&db), root, &ipc_name, None);
    let ct = CancellationToken::new();
    spawn_ipc_server(Arc::clone(&state), ct.clone()).expect("spawn ipc server");

    let resp = send_ipc(
        ipc_name,
        serde_json::json!({"command": "mode", "mode": "hybrid"}),
    )
    .await;
    ct.cancel();

    assert!(
        resp["ok"].as_bool().unwrap_or(false),
        "mode command should succeed: {resp}"
    );
    assert_eq!(resp["data"]["current_mode"], "hybrid");

    // Verify the mode was updated in the DB.
    let updated = SessionRepo::new(Arc::clone(&db))
        .get_by_id(&session.id)
        .await
        .expect("get session")
        .expect("session exists");
    assert_eq!(
        updated.mode,
        SessionMode::Hybrid,
        "session mode should be Hybrid in DB"
    );
}
