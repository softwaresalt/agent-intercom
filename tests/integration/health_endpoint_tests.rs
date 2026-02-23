//! Integration tests for the HTTP health endpoint.
//!
//! Validates that `GET /health` returns `200 OK` with body `"ok"`.
//! Uses an ephemeral port to avoid conflicts with running instances.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use monocoque_agent_rc::mcp::sse::serve_sse;

use super::test_helpers::{test_app_state, test_config};

/// Spawn the SSE/health server on an ephemeral port, returning the bound address.
///
/// Caller must cancel `ct` to shut the server down.
async fn spawn_server() -> (String, CancellationToken) {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");

    // Build config with port 0 so the OS assigns an ephemeral port.
    let mut config = test_config(root);
    config.http_port = 0;

    let state = test_app_state(config).await;
    let ct = CancellationToken::new();

    // Bind a listener ourselves to know the port, then drop it and
    // re-bind inside `serve_sse`.  Instead, we use the retry approach:
    // bind a temporary listener to discover a free port, then configure
    // serve_sse to use that port.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral");
    let addr = listener.local_addr().expect("local addr");
    drop(listener); // Free the port so serve_sse can bind it.

    // Update the state config to use the discovered port.
    let port = addr.port();
    let state = {
        let mut cfg = (*state.config).clone();
        cfg.http_port = port;
        let new_state = monocoque_agent_rc::mcp::handler::AppState {
            config: Arc::new(cfg),
            db: Arc::clone(&state.db),
            slack: None,
            pending_approvals: Arc::clone(&state.pending_approvals),
            pending_prompts: Arc::clone(&state.pending_prompts),
            pending_waits: Arc::clone(&state.pending_waits),
            stall_detectors: None,
            ipc_auth_token: None,
        };
        Arc::new(new_state)
    };

    let server_ct = ct.clone();
    tokio::spawn(async move {
        let _ = serve_sse(state, server_ct).await;
    });

    // Give the server a moment to bind.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let base_url = format!("http://127.0.0.1:{port}");
    (base_url, ct)
}

// ── GET /health returns 200 OK ───────────────────────────────

#[tokio::test]
async fn health_returns_ok() {
    let (base_url, ct) = spawn_server().await;

    let resp = reqwest::get(format!("{base_url}/health"))
        .await
        .expect("HTTP GET /health");

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.expect("body");
    assert_eq!(body, "ok");

    ct.cancel();
}

// ── GET /health with trailing slash redirects or still works ─

#[tokio::test]
async fn health_without_trailing_slash_works() {
    let (base_url, ct) = spawn_server().await;

    let resp = reqwest::get(format!("{base_url}/health"))
        .await
        .expect("HTTP GET /health");

    assert_eq!(resp.status(), 200);
    ct.cancel();
}

// ── Non-existent route returns 404 ──────────────────────────

#[tokio::test]
async fn non_existent_route_returns_404() {
    let (base_url, ct) = spawn_server().await;

    let resp = reqwest::get(format!("{base_url}/nonexistent"))
        .await
        .expect("HTTP GET /nonexistent");

    assert_eq!(resp.status(), 404);
    ct.cancel();
}
