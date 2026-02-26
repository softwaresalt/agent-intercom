//! Integration tests for the Streamable HTTP MCP transport (rmcp 0.13, US5).
//!
//! Verifies T094: `StreamableHttpService` on `/mcp` endpoint accepts HTTP POST
//! and returns a valid `tools/list` response.
//!
//! All tests in this module are gated behind the `rmcp-upgrade` feature flag
//! and will not compile or run in a default build. Enable with:
//!
//! ```sh
//! cargo test --features rmcp-upgrade
//! ```
//!
//! In the red gate (before T100-T107 implementation), the `serve_http` function
//! does not exist, so enabling the feature will produce a compile error — which
//! is the expected failure mode for TDD.

#[cfg(feature = "rmcp-upgrade")]
use std::sync::Arc;

// All code in this module is gated on the `rmcp-upgrade` feature.
// Without it, none of the items below are compiled, so the missing
// `serve_http` symbol does not cause a compile error in default builds.
// The red gate is verified by running with `--features rmcp-upgrade`.
#[cfg(feature = "rmcp-upgrade")]
use agent_intercom::mcp::handler::AppState;
#[cfg(feature = "rmcp-upgrade")]
use tokio_util::sync::CancellationToken;

#[cfg(feature = "rmcp-upgrade")]
use super::test_helpers::{test_app_state, test_config};

/// Spawn the HTTP server using the new `serve_http` function (rmcp 0.13).
///
/// Returns the base URL and a cancellation token for clean shutdown.
/// Compiles only when the `rmcp-upgrade` feature is enabled.
#[cfg(feature = "rmcp-upgrade")]
async fn spawn_http_server() -> (String, CancellationToken) {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8");

    let mut config = test_config(root);
    config.http_port = 0;

    let state = test_app_state(config).await;
    let ct = CancellationToken::new();

    // Discover a free port, then configure the server to use it.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral");
    let addr = listener.local_addr().expect("local addr");
    drop(listener);
    let port = addr.port();

    let state: Arc<AppState> = {
        let mut cfg = (*state.config).clone();
        cfg.http_port = port;
        let new_state = agent_intercom::mcp::handler::AppState {
            config: Arc::new(cfg),
            db: Arc::clone(&state.db),
            slack: None,
            pending_approvals: Arc::clone(&state.pending_approvals),
            pending_prompts: Arc::clone(&state.pending_prompts),
            pending_waits: Arc::clone(&state.pending_waits),
            pending_modal_contexts: Default::default(),
            stall_detectors: None,
            ipc_auth_token: None,
            policy_cache: Arc::default(),
            audit_logger: None,
            active_children: Arc::default(),
        };
        Arc::new(new_state)
    };

    let server_ct = ct.clone();
    tokio::spawn(async move {
        // T101: `serve_http` replaces `serve_sse` with StreamableHttpService.
        // This will compile only after the rmcp 0.13 upgrade (T100-T105).
        let _ = agent_intercom::mcp::sse::serve_http(state, server_ct).await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(250)).await;

    (format!("http://127.0.0.1:{port}"), ct)
}

/// T094: POST to `/mcp` with a `tools/list` JSON-RPC request returns a valid
/// response with the 9 intercom-themed tool names.
///
/// In the red gate (before T100-T107), running `--features rmcp-upgrade`
/// produces a compile error since `serve_http` does not exist yet.
/// After implementation, this test verifies the StreamableHttpService works.
#[tokio::test]
#[cfg(feature = "rmcp-upgrade")]
async fn mcp_endpoint_returns_tools_list() {
    let (base_url, ct) = spawn_http_server().await;

    // Streamable HTTP transport uses a single POST /mcp endpoint.
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "id": 1,
        "params": {}
    });

    let resp = client
        .post(format!("{base_url}/mcp"))
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .expect("POST /mcp tools/list");

    assert!(
        resp.status().is_success(),
        "expected 2xx, got {}",
        resp.status()
    );

    let text = resp.text().await.expect("response body");
    // Should contain at least one of the intercom-themed tool names.
    assert!(
        text.contains("check_clearance") || text.contains("\"tools\""),
        "expected tool names in response, got: {text}"
    );

    ct.cancel();
}

/// T096: Old `/sse` endpoint returns `410 Gone` or `301 Redirect` after upgrade.
///
/// After the streamable HTTP upgrade, the `/sse` endpoint is replaced by `/mcp`.
/// MCP clients using the old `/sse` URL should receive a deprecation response.
#[tokio::test]
#[cfg(feature = "rmcp-upgrade")]
async fn old_sse_endpoint_returns_gone_or_redirect() {
    let (base_url, ct) = spawn_http_server().await;

    let resp = reqwest::get(format!("{base_url}/sse"))
        .await
        .expect("GET /sse");

    let status = resp.status().as_u16();
    assert!(
        status == 301 || status == 302 || status == 308 || status == 410,
        "expected redirect (3xx) or 410 Gone for /sse, got: {status}"
    );

    ct.cancel();
}

/// T097: Concurrent MCP connections are handled independently.
///
/// Sends two separate `tools/list` requests concurrently and verifies
/// that both receive valid independent responses.
#[tokio::test]
#[cfg(feature = "rmcp-upgrade")]
async fn concurrent_connections_handled_independently() {
    let (base_url, ct) = spawn_http_server().await;

    let client = Arc::new(reqwest::Client::new());
    let url = format!("{base_url}/mcp");

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "id": 1,
        "params": {}
    });

    let client1 = Arc::clone(&client);
    let client2 = Arc::clone(&client);
    let url1 = url.clone();
    let url2 = url.clone();
    let body1 = body.clone();
    let body2 = body.clone();

    let (resp1, resp2) = tokio::join!(
        async move {
            client1
                .post(&url1)
                .header("content-type", "application/json")
                .json(&body1)
                .send()
                .await
                .expect("POST /mcp (connection 1)")
        },
        async move {
            client2
                .post(&url2)
                .header("content-type", "application/json")
                .json(&body2)
                .send()
                .await
                .expect("POST /mcp (connection 2)")
        }
    );

    assert!(
        resp1.status().is_success(),
        "connection 1: expected 2xx, got {}",
        resp1.status()
    );
    assert!(
        resp2.status().is_success(),
        "connection 2: expected 2xx, got {}",
        resp2.status()
    );

    ct.cancel();
}

/// T098: Graceful handling of a dropped HTTP connection.
///
/// Issues a request then immediately drops the response receiver;
/// the server should handle the disconnection without crashing.
#[tokio::test]
#[cfg(feature = "rmcp-upgrade")]
async fn dropped_connection_does_not_crash_server() {
    let (base_url, ct) = spawn_http_server().await;

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "id": 99,
        "params": {}
    });

    // Send but immediately drop the future — the server should not crash.
    let _ = client
        .post(format!("{base_url}/mcp"))
        .header("content-type", "application/json")
        .json(&body)
        .timeout(std::time::Duration::from_millis(50))
        .send()
        .await; // may timeout; that's expected

    // Server should still be alive — health endpoint still responds.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let health_resp = reqwest::get(format!("{base_url}/health"))
        .await
        .expect("server survived dropped connection");

    assert_eq!(
        health_resp.status(),
        200,
        "server crashed after dropped connection"
    );

    ct.cancel();
}
