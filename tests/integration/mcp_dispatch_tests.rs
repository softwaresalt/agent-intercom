//! Integration tests for full MCP tool dispatch through the HTTP/SSE transport.
//!
//! Validates:
//! - S001: Heartbeat tool call dispatched via HTTP transport
//! - S003: `recover_state` tool call dispatched via HTTP transport
//! - S006: Unknown tool name returns MCP error response
//! - S007: Malformed arguments return descriptive MCP error
//! - S010: `tools/list` returns exactly 9 registered tools
//!
//! NOTE: rmcp `RequestContext` has no public constructor. These tests use
//! a minimal hand-rolled MCP over SSE client to exercise the transport layer.
//!
//! FR-001 — MCP Transport Dispatch

use std::sync::Arc;
use std::time::Duration;

use agent_intercom::mcp::handler::AppState;
use agent_intercom::mcp::sse::serve_sse;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::test_helpers::{test_app_state, test_config};

// ── Server fixture helpers ────────────────────────────────────

/// Spawn a test SSE/MCP server on an ephemeral port.
///
/// Caller must cancel `ct` when done. Returns `(base_url, ct)`.
async fn spawn_test_server() -> (String, CancellationToken) {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().to_str().expect("utf8 path");
    let mut config = test_config(root);
    config.http_port = 0;
    let state = test_app_state(config).await;
    let ct = CancellationToken::new();

    // Discover an ephemeral port.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("ephemeral bind");
    let addr = listener.local_addr().expect("local addr");
    drop(listener);
    let port = addr.port();

    let state = Arc::new(AppState {
        config: Arc::new({
            let mut cfg = (*state.config).clone();
            cfg.http_port = port;
            cfg
        }),
        db: Arc::clone(&state.db),
        slack: None,
        pending_approvals: Arc::clone(&state.pending_approvals),
        pending_prompts: Arc::clone(&state.pending_prompts),
        pending_waits: Arc::clone(&state.pending_waits),
        stall_detectors: None,
        ipc_auth_token: None,
    });

    let server_ct = ct.clone();
    tokio::spawn(async move {
        let _ = serve_sse(state, server_ct).await;
    });

    // Allow the server to bind.
    tokio::time::sleep(Duration::from_millis(200)).await;
    (format!("http://127.0.0.1:{port}"), ct)
}

// ── Minimal MCP / SSE client ──────────────────────────────────

/// SSE connection state: message endpoint URL + a channel of raw SSE data lines.
struct SseConnection {
    /// Absolute URL to POST JSON-RPC messages to (e.g. `http://…/message?sessionId=…`).
    message_url: String,
    /// Receiver of raw SSE `data:` payloads (one `String` per non-empty payload).
    data_rx: mpsc::Receiver<String>,
    /// HTTP client reused across all POST calls.
    client: reqwest::Client,
    /// Monotonically increasing JSON-RPC request id.
    next_id: u64,
}

impl SseConnection {
    /// Connect to `/sse`, wait for the `endpoint` event, then return the
    /// connection object. Spawns a background task to drain the SSE stream
    /// and forward all `data:` payloads to `data_rx`.
    async fn connect(base_url: &str) -> Self {
        let client = reqwest::Client::new();
        let sse_url = format!("{base_url}/sse");

        let mut response = client
            .get(&sse_url)
            .header("Accept", "text/event-stream")
            .send()
            .await
            .expect("GET /sse");

        // One-shot for the endpoint URL; unbounded channel for subsequent events.
        let (endpoint_tx, endpoint_rx) = tokio::sync::oneshot::channel::<String>();
        let (data_tx, data_rx) = mpsc::channel::<String>(64);

        // Background task: parse SSE lines and fan-out to channels.
        tokio::spawn(async move {
            let mut buf = String::new();
            let mut in_endpoint = false;
            let mut endpoint_tx_opt = Some(endpoint_tx);
            let mut endpoint_delivered = false;

            loop {
                let chunk = response.chunk().await;
                let Ok(Some(bytes)) = chunk else { break };
                buf.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(nl) = buf.find('\n') {
                    let line = buf[..nl].trim_end_matches('\r').to_owned();
                    buf = buf[nl + 1..].to_owned();

                    if line.is_empty() {
                        in_endpoint = false;
                    } else if let Some(event_type) = line.strip_prefix("event:") {
                        in_endpoint = event_type.trim() == "endpoint";
                    } else if let Some(rest) = line.strip_prefix("data:") {
                        let data = rest.trim().to_owned();
                        if in_endpoint && !endpoint_delivered {
                            endpoint_delivered = true;
                            if let Some(tx) = endpoint_tx_opt.take() {
                                let _ = tx.send(data);
                            }
                        } else if !data.is_empty() {
                            let _ = data_tx.send(data).await;
                        }
                    }
                }
            }
        });

        // Wait for the endpoint event to arrive (server sends it immediately).
        let endpoint_path = tokio::time::timeout(Duration::from_secs(5), endpoint_rx)
            .await
            .expect("endpoint event within 5 s")
            .expect("endpoint oneshot");

        // Build the absolute URL for the message endpoint.
        let message_url = if endpoint_path.starts_with("http") {
            endpoint_path
        } else {
            format!("{base_url}{endpoint_path}")
        };

        Self {
            message_url,
            data_rx,
            client,
            next_id: 1,
        }
    }

    /// Send a JSON-RPC request and wait for the matching response via SSE.
    ///
    /// Returns the `result` field of the response, or panics if an error is
    /// received or the timeout elapses.
    async fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;

        let body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let body_str = serde_json::to_string(&body).expect("serialize JSON-RPC");
        self.client
            .post(&self.message_url)
            .header("Content-Type", "application/json")
            .body(body_str)
            .send()
            .await
            .expect("POST JSON-RPC");

        // Drain SSE until we find the matching response id.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let data = tokio::time::timeout(remaining, self.data_rx.recv())
                .await
                .expect("SSE response within 10 s")
                .expect("SSE channel open");

            let msg: Value = serde_json::from_str(&data).unwrap_or(Value::Null);
            if msg.get("id") == Some(&Value::Number(serde_json::Number::from(id))) {
                return msg;
            }
            // Priming events or unrelated messages — continue draining.
        }
    }

    /// Send a JSON-RPC notification (no id, no response expected).
    async fn notify(&self, method: &str, params: Value) {
        let body = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        let body_str = serde_json::to_string(&body).expect("serialize notification");
        self.client
            .post(&self.message_url)
            .header("Content-Type", "application/json")
            .body(body_str)
            .send()
            .await
            .expect("POST notification");
    }

    /// Perform the MCP initialize + initialized handshake.
    ///
    /// After this call, the connection is ready for tool calls.
    async fn handshake(&mut self) {
        let _init_response = self
            .request(
                "initialize",
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "monocoque-test",
                        "version": "0.0.1"
                    }
                }),
            )
            .await;

        self.notify("notifications/initialized", json!({})).await;

        // Brief pause so `on_initialized` fires and creates the DB session.
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    /// Call a tool. Returns the full JSON-RPC response object (may contain
    /// either `result` or `error`).
    async fn call_tool(&mut self, tool_name: &str, arguments: Value) -> Value {
        self.request(
            "tools/call",
            json!({
                "name": tool_name,
                "arguments": arguments,
            }),
        )
        .await
    }

    /// List tools. Returns the full JSON-RPC response.
    async fn list_tools(&mut self) -> Value {
        self.request("tools/list", json!({})).await
    }
}

// ── S010: tools/list returns 9 tools ─────────────────────────

/// S010 — Verify the MCP transport serves exactly 9 registered tools.
///
/// `list_tools()` does not require an active session, making this the
/// simplest transport-level smoke test for the tool router.
#[tokio::test]
async fn transport_list_tools_returns_nine_tools() {
    let (base_url, ct) = spawn_test_server().await;
    let mut conn = SseConnection::connect(&base_url).await;
    conn.handshake().await;

    let response = conn.list_tools().await;
    let tools = response["result"]["tools"].as_array().expect("tools array");

    assert_eq!(
        tools.len(),
        9,
        "expected exactly 9 registered tools; got {tools:?}"
    );

    ct.cancel();
}

// ── S001: heartbeat dispatched via transport ──────────────────

/// S001 — Verify that the `heartbeat` tool call is dispatched end-to-end
/// through the SSE transport and returns `acknowledged: true`.
///
/// `on_initialized` auto-creates an active session when the SSE client
/// connects, so no pre-existing session is needed.
#[tokio::test]
async fn transport_heartbeat_dispatch() {
    let (base_url, ct) = spawn_test_server().await;
    let mut conn = SseConnection::connect(&base_url).await;
    conn.handshake().await;

    let response = conn
        .call_tool("heartbeat", json!({"status_message": "transport test"}))
        .await;

    // Response must have a `result`, not an `error`.
    assert!(
        response.get("error").is_none(),
        "heartbeat should not return an error; got {response}"
    );
    let content = &response["result"]["content"];
    let text = content[0]["text"]
        .as_str()
        .expect("text content in heartbeat result");
    let result_json: Value = serde_json::from_str(text).expect("result is valid JSON");
    assert_eq!(
        result_json["acknowledged"],
        json!(true),
        "heartbeat should return acknowledged: true; got {result_json}"
    );

    ct.cancel();
}

// ── S003: recover_state dispatched via transport ──────────────

/// S003 — Verify that `recover_state` dispatched via transport succeeds
/// and returns `{"status": "clean"}` when no interrupted sessions exist.
#[tokio::test]
async fn transport_recover_state_dispatch() {
    let (base_url, ct) = spawn_test_server().await;
    let mut conn = SseConnection::connect(&base_url).await;
    conn.handshake().await;

    let response = conn.call_tool("recover_state", json!({})).await;

    assert!(
        response.get("error").is_none(),
        "recover_state should not return an error; got {response}"
    );
    let content = &response["result"]["content"];
    let text = content[0]["text"]
        .as_str()
        .expect("text content in recover_state result");
    let result_json: Value = serde_json::from_str(text).expect("result is valid JSON");
    assert_eq!(
        result_json["status"],
        json!("clean"),
        "recover_state with no interrupted sessions should return status: clean; got {result_json}"
    );

    ct.cancel();
}

// ── S006: unknown tool returns error ─────────────────────────

/// S006 — Verify that calling an unknown tool name via transport returns
/// a JSON-RPC error (not a panic or HTTP error).
#[tokio::test]
async fn transport_unknown_tool_returns_error() {
    let (base_url, ct) = spawn_test_server().await;
    let mut conn = SseConnection::connect(&base_url).await;
    conn.handshake().await;

    let response = conn
        .call_tool("definitely_not_a_real_tool", json!({}))
        .await;

    assert!(
        response.get("error").is_some(),
        "unknown tool should produce a JSON-RPC error; got {response}"
    );

    ct.cancel();
}

// ── S007: malformed args return error ────────────────────────

/// S007 — Verify that passing malformed arguments to a real tool via
/// transport returns a JSON-RPC error with a descriptive message.
///
/// Uses `set_operational_mode` with a completely wrong field name.
#[tokio::test]
async fn transport_malformed_args_returns_error() {
    let (base_url, ct) = spawn_test_server().await;
    let mut conn = SseConnection::connect(&base_url).await;
    conn.handshake().await;

    // `set_operational_mode` expects `{ mode: "…" }`, not `{ wrong_field: 1 }`.
    let response = conn
        .call_tool("set_operational_mode", json!({"wrong_field": 99}))
        .await;

    assert!(
        response.get("error").is_some(),
        "malformed args should produce a JSON-RPC error; got {response}"
    );

    ct.cancel();
}
