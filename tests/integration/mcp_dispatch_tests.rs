//! Integration tests for full MCP tool dispatch through the HTTP/Streamable-HTTP transport.
//!
//! Validates:
//! - S001: Heartbeat tool call dispatched via HTTP transport
//! - S003: `recover_state` tool call dispatched via HTTP transport
//! - S006: Unknown tool name returns MCP error response
//! - S007: Malformed arguments return descriptive MCP error
//! - S010: `tools/list` returns exactly 9 registered tools
//!
//! Uses the rmcp 0.13 Streamable HTTP protocol: POST to `/mcp` for every
//! request, with the `Mcp-Session-Id` header on subsequent requests.
//!
//! FR-001 — MCP Transport Dispatch

use std::sync::Arc;
use std::time::Duration;

use agent_intercom::mcp::handler::AppState;
use agent_intercom::mcp::sse::serve_http;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::test_helpers::{test_app_state, test_config};

// ── Server fixture helpers ────────────────────────────────────

/// Spawn a test HTTP/MCP server on an ephemeral port.
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
        pending_modal_contexts: Arc::default(),
        stall_detectors: None,
        ipc_auth_token: None,
        policy_cache: Arc::default(),
        audit_logger: None,
        active_children: Arc::default(),
        pending_command_approvals: Arc::clone(&state.pending_command_approvals),
        stall_event_tx: None,
        driver: agent_intercom::driver::mcp_driver::McpDriver::new_empty(),
    });

    let server_ct = ct.clone();
    tokio::spawn(async move {
        let _ = serve_http(state, server_ct).await;
    });

    // Allow the server to bind.
    tokio::time::sleep(Duration::from_millis(200)).await;
    (format!("http://127.0.0.1:{port}"), ct)
}

// ── Minimal MCP / Streamable-HTTP client ─────────────────────

/// Streamable-HTTP MCP connection state.
///
/// Uses the rmcp 0.13 protocol: every request is a POST to `/mcp`. After the
/// `initialize` request the server returns an `Mcp-Session-Id` header that
/// must be included in all subsequent requests.
struct McpConnection {
    /// Absolute URL for all MCP requests (e.g. `http://…/mcp`).
    mcp_url: String,
    /// Session ID obtained from the `Mcp-Session-Id` response header.
    session_id: Option<String>,
    /// HTTP client reused across all requests.
    client: reqwest::Client,
    /// Monotonically increasing JSON-RPC request id.
    next_id: u64,
}

impl McpConnection {
    /// Create a new (uninitialized) connection to the `/mcp` endpoint.
    fn new(base_url: &str) -> Self {
        Self {
            mcp_url: format!("{base_url}/mcp"),
            session_id: None,
            client: reqwest::Client::new(),
            next_id: 1,
        }
    }

    /// Perform the MCP initialize + initialized handshake.
    ///
    /// Returns the raw `initialize` JSON-RPC response for inspection.
    /// After this call, the session ID is set and the connection is ready.
    async fn handshake(&mut self) -> Value {
        let id = self.next_id;
        self.next_id += 1;

        let body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "intercom-test",
                    "version": "0.0.1"
                }
            }
        });

        let response = self
            .client
            .post(&self.mcp_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .body(serde_json::to_string(&body).expect("serialize"))
            .send()
            .await
            .expect("POST /mcp initialize");

        // Capture the session ID from the response header.
        if let Some(sid) = response.headers().get("mcp-session-id") {
            self.session_id = sid.to_str().ok().map(ToOwned::to_owned);
        }

        // Read and parse the response body.
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let text = response.text().await.expect("initialize response body");
        let json_str = if content_type.contains("text/event-stream") || text.contains("\ndata:") {
            text.lines()
                .find_map(|line| {
                    line.strip_prefix("data:")
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                })
                .unwrap_or("")
                .to_owned()
        } else {
            text
        };
        let init_response: Value = serde_json::from_str(&json_str).unwrap_or(Value::Null);

        // Send the initialized notification.
        self.notify("notifications/initialized", json!({})).await;

        // Brief pause so `on_initialized` fires and creates the DB session.
        tokio::time::sleep(Duration::from_millis(100)).await;

        init_response
    }

    /// Send a JSON-RPC request and wait for the response.
    ///
    /// Returns the full JSON-RPC response object.
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

        let mut req = self
            .client
            .post(&self.mcp_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .body(body_str);

        if let Some(sid) = &self.session_id {
            req = req.header("mcp-session-id", sid.clone());
        }

        let response = req.send().await.expect("POST JSON-RPC");

        // Read and parse the response body.
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let text = response.text().await.expect("response body");

        // The response may be plain JSON or a text/event-stream payload.
        // For SSE, find the first non-empty `data:` line.
        let json_str = if content_type.contains("text/event-stream") || text.contains("\ndata:") {
            text.lines()
                .find_map(|line| {
                    line.strip_prefix("data:")
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                })
                .unwrap_or("")
                .to_owned()
        } else {
            text
        };

        serde_json::from_str(&json_str).unwrap_or(Value::Null)
    }

    /// Send a JSON-RPC notification (no id, no response expected).
    async fn notify(&self, method: &str, params: Value) {
        let body = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        let body_str = serde_json::to_string(&body).expect("serialize notification");

        let mut req = self
            .client
            .post(&self.mcp_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .body(body_str);

        if let Some(sid) = &self.session_id {
            req = req.header("mcp-session-id", sid.clone());
        }

        let _ = req.send().await;
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
    let mut conn = McpConnection::new(&base_url);
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
    let mut conn = McpConnection::new(&base_url);
    conn.handshake().await;

    let response = conn
        .call_tool("ping", json!({"status_message": "transport test"}))
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

/// S003 — Verify that `reboot` (`recover_state`) dispatched via transport succeeds
/// and returns `{"status": "clean"}` when no interrupted sessions exist.
#[tokio::test]
async fn transport_recover_state_dispatch() {
    let (base_url, ct) = spawn_test_server().await;
    let mut conn = McpConnection::new(&base_url);
    conn.handshake().await;

    let response = conn.call_tool("reboot", json!({})).await;

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
    let mut conn = McpConnection::new(&base_url);
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
    let mut conn = McpConnection::new(&base_url);
    conn.handshake().await;

    // `switch_freq` expects `{ mode: "…" }`, not `{ wrong_field: 1 }`.
    let response = conn
        .call_tool("switch_freq", json!({"wrong_field": 99}))
        .await;

    assert!(
        response.get("error").is_some(),
        "malformed args should produce a JSON-RPC error; got {response}"
    );

    ct.cancel();
}
// ── T033: tools/list returns 9 new intercom-themed names ─────

/// T033 — Verify `tools/list` returns exactly 9 tools with the new
/// intercom-themed names (`check_clearance`, `check_diff`, `auto_check`,
/// `transmit`, `standby`, `ping`, `broadcast`, `reboot`, `switch_freq`).
#[tokio::test]
async fn transport_list_tools_uses_new_intercom_names() {
    let (base_url, ct) = spawn_test_server().await;
    let mut conn = McpConnection::new(&base_url);
    conn.handshake().await;

    let response = conn.list_tools().await;
    let tools = response["result"]["tools"].as_array().expect("tools array");

    let expected_names: std::collections::HashSet<&str> = [
        "check_clearance",
        "check_diff",
        "auto_check",
        "transmit",
        "standby",
        "ping",
        "broadcast",
        "reboot",
        "switch_freq",
    ]
    .iter()
    .copied()
    .collect();

    let actual_names: std::collections::HashSet<&str> =
        tools.iter().filter_map(|t| t["name"].as_str()).collect();

    assert_eq!(
        actual_names, expected_names,
        "tools/list should return exactly the 9 intercom-themed names; got {actual_names:?}"
    );

    ct.cancel();
}

// ── T034: ServerInfo reports "agent-intercom" ─────────────────

/// T034 — Verify the MCP `initialize` handshake response contains
/// `serverInfo.name == "agent-intercom"` and a non-empty `version`.
#[tokio::test]
async fn transport_server_info_reports_agent_intercom() {
    let (base_url, ct) = spawn_test_server().await;
    let mut conn = McpConnection::new(&base_url);

    // handshake() performs initialize + initialized and returns the init response.
    let init_response = conn.handshake().await;

    let server_info = &init_response["result"]["serverInfo"];
    assert_eq!(
        server_info["name"].as_str(),
        Some("agent-intercom"),
        "serverInfo.name should be 'agent-intercom'; got {server_info}"
    );
    assert!(
        !server_info["version"].as_str().unwrap_or("").is_empty(),
        "serverInfo.version should be non-empty; got {server_info}"
    );

    ct.cancel();
}

// ── T035: Old tool name is rejected ──────────────────────────

/// T035 — Verify that calling the old tool name `ask_approval` via
/// the transport returns a JSON-RPC error (tool not found).
#[tokio::test]
async fn transport_old_tool_name_returns_error() {
    let (base_url, ct) = spawn_test_server().await;
    let mut conn = McpConnection::new(&base_url);
    conn.handshake().await;

    let response = conn
        .call_tool(
            "ask_approval",
            json!({
                "title": "test",
                "diff": "--- a\n+++ b\n@@ -1 +1 @@\n-old\n+new",
                "file_path": "src/main.rs"
            }),
        )
        .await;

    assert!(
        response.get("error").is_some(),
        "old tool name 'ask_approval' should produce a JSON-RPC error; got {response}"
    );

    ct.cancel();
}

// ── T037: Empty tool name returns error ───────────────────────

/// T037 — Verify that `call_tool` with an empty string tool name
/// returns a JSON-RPC error rather than panicking.
#[tokio::test]
async fn transport_empty_tool_name_returns_error() {
    let (base_url, ct) = spawn_test_server().await;
    let mut conn = McpConnection::new(&base_url);
    conn.handshake().await;

    let response = conn.call_tool("", json!({})).await;

    assert!(
        response.get("error").is_some(),
        "empty tool name should produce a JSON-RPC error; got {response}"
    );

    ct.cancel();
}
