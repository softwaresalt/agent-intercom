//! Integration tests for ACP subprocess → MCP HTTP bridge (Phase 13, T107–T109).
//!
//! Validates HITL-003: ACP subprocesses MUST be able to reach MCP tools via
//! the HTTP transport when the server is running in ACP mode.
//!
//! Covers:
//! - T107 / S077: ACP mode HTTP server is accessible (health endpoint returns 200)
//! - T108 / S078: Requests without a valid `session_id` are rejected with 401
//! - T109 / S079: Requests with a valid active `session_id` are allowed through

use std::sync::Arc;

use agent_intercom::mcp::sse::serve_with_listener;
use agent_intercom::mode::ServerMode;
use agent_intercom::models::session::{Session, SessionMode, SessionStatus};
use agent_intercom::persistence::session_repo::SessionRepo;
use tokio_util::sync::CancellationToken;

use super::test_helpers::{test_app_state, test_config};

/// Spawn an HTTP server in ACP mode and return (`base_url`, ct, state).
async fn spawn_acp_http_server() -> (
    String,
    CancellationToken,
    Arc<agent_intercom::mcp::handler::AppState>,
) {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");

    let config = test_config(root);
    let state = test_app_state(config).await;

    // Bind an ephemeral port first so we know the URL.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral");
    let addr = listener.local_addr().expect("local addr");
    let port = addr.port();

    // Build an ACP-mode AppState sharing the same DB.
    let acp_state = Arc::new(agent_intercom::mcp::handler::AppState {
        config: {
            let mut cfg = (*state.config).clone();
            cfg.http_port = port;
            Arc::new(cfg)
        },
        db: Arc::clone(&state.db),
        slack: None,
        pending_approvals: Arc::clone(&state.pending_approvals),
        pending_prompts: Arc::clone(&state.pending_prompts),
        pending_waits: Arc::clone(&state.pending_waits),
        pending_modal_contexts: Arc::default(),
        pending_thread_replies: Arc::default(),
        stall_detectors: None,
        ipc_auth_token: None,
        policy_cache: Arc::default(),
        audit_logger: None,
        active_children: Arc::default(),
        pending_command_approvals: Arc::clone(&state.pending_command_approvals),
        stall_event_tx: None,
        driver: agent_intercom::driver::mcp_driver::McpDriver::new_empty(),
        server_mode: ServerMode::Acp, // ACP mode — the key difference
        workspace_mappings: Arc::default(),
        acp_event_tx: None,
        acp_driver: None,
    });

    let ct = CancellationToken::new();
    let server_ct = ct.clone();
    let srv_state = Arc::clone(&acp_state);
    tokio::spawn(async move {
        let _ = serve_with_listener(listener, srv_state, server_ct).await;
    });

    // Give the server a moment to start accepting connections.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let base_url = format!("http://127.0.0.1:{port}");
    (base_url, ct, acp_state)
}

// ── T107 / S077 ────────────────────────────────────────────────────────────────

/// ACP mode: the HTTP MCP transport MUST start so that ACP subprocesses can
/// call MCP tools like `check_clearance`.
///
/// The /health endpoint serves as a simple liveness probe — if it returns 200,
/// the HTTP server is up and the transport is running in ACP mode.
#[tokio::test]
async fn acp_mode_http_transport_is_accessible() {
    let (base_url, ct, _state) = spawn_acp_http_server().await;

    let resp = reqwest::get(format!("{base_url}/health"))
        .await
        .expect("GET /health");

    assert_eq!(
        resp.status(),
        200,
        "ACP mode HTTP server must be accessible"
    );
    let body = resp.text().await.expect("body");
    assert_eq!(body, "ok");

    ct.cancel();
}

// ── T108 / S078 ────────────────────────────────────────────────────────────────

/// ACP mode: requests to /mcp without a `session_id` query parameter MUST be
/// rejected with HTTP 401 (HITL-003 security requirement).
#[tokio::test]
async fn acp_mode_mcp_request_without_session_id_is_rejected() {
    let (base_url, ct, _state) = spawn_acp_http_server().await;

    // POST to /mcp without session_id — must be rejected.
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base_url}/mcp"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .body(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}"#)
        .send()
        .await
        .expect("POST /mcp");

    assert_eq!(
        resp.status(),
        401,
        "ACP mode: POST to /mcp without session_id must return 401"
    );

    ct.cancel();
}

/// ACP mode: requests to /mcp with an invalid/unknown `session_id` MUST be
/// rejected with HTTP 401 (HITL-003 security requirement).
#[tokio::test]
async fn acp_mode_mcp_request_with_invalid_session_id_is_rejected() {
    let (base_url, ct, _state) = spawn_acp_http_server().await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base_url}/mcp?session_id=nonexistent-session-id"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .body(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}"#)
        .send()
        .await
        .expect("POST /mcp with invalid session_id");

    assert_eq!(
        resp.status(),
        401,
        "ACP mode: POST to /mcp with invalid session_id must return 401"
    );

    ct.cancel();
}

// ── T109 / S079 ────────────────────────────────────────────────────────────────

/// ACP mode: requests to /mcp with a valid active `session_id` MUST be allowed
/// through to the MCP layer.
///
/// We can't verify a full MCP tool call here (requires the whole rmcp
/// handshake), but we can verify the request is not rejected at the auth
/// layer by checking the response is NOT a 401.
#[tokio::test]
async fn acp_mode_mcp_request_with_valid_session_id_is_allowed() {
    let (base_url, ct, state) = spawn_acp_http_server().await;

    // Create an active session in the DB.
    let repo = SessionRepo::new(Arc::clone(&state.db));
    let session = Session::new(
        "U_TEST".into(),
        "/workspace".into(),
        Some("test session".into()),
        SessionMode::Remote,
    );
    let created = repo.create(&session).await.expect("create session");
    let active = repo
        .update_status(&created.id, SessionStatus::Active)
        .await
        .expect("activate session");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base_url}/mcp?session_id={}", active.id))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .body(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}"#)
        .send()
        .await
        .expect("POST /mcp with valid session_id");

    assert_ne!(
        resp.status(),
        401,
        "ACP mode: POST to /mcp with valid session_id must NOT return 401"
    );

    ct.cancel();
}
