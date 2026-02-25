//! HTTP/Streamable-HTTP transport for multi-agent connections.
//!
//! Mounts a [`StreamableHttpService`] behind an axum router so that remote
//! agents can connect via the MCP Streamable-HTTP transport (rmcp 0.13+).
//!
//! The `/mcp` endpoint accepts an optional `channel_id` query parameter
//! (e.g. `/mcp?channel_id=C_WORKSPACE_CHANNEL`) so that each connected
//! workspace can target a different Slack channel.
//!
//! The legacy `/sse` and `/message` endpoints return `410 Gone` to inform
//! clients that they must upgrade to the `/mcp` endpoint.
//!
//! ## Accept header middleware
//!
//! VS Code may send `Accept: application/json` without `text/event-stream`.
//! The rmcp `StreamableHttpService` requires both. The
//! [`ensure_accept_header`] middleware patches the header before it reaches
//! rmcp, avoiding a 406 rejection.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use super::handler::{AppState, IntercomServer};
use crate::{AppError, Result};

/// Handler for `GET /health` — returns 200 OK with a plain-text body.
///
/// Useful for probing liveness without initiating an MCP session.
async fn health() -> &'static str {
    "ok"
}

/// Middleware that normalizes inbound MCP requests for rmcp 0.13
/// compatibility.
///
/// Performs two critical fixups:
///
/// 1. **Accept header**: Ensures both `application/json` and
///    `text/event-stream` are present so rmcp does not return 406.
///
/// 2. **Initialize params sanitization**: VS Code 1.109+ sends
///    `protocolVersion: "2025-11-25"` and extra capability fields
///    (`extensions`) that rmcp 0.13 cannot deserialize, causing the
///    request to be parsed as a `CustomRequest` instead of
///    `InitializeRequest`. The middleware strips unknown capability
///    fields and downgrades the protocol version to `"2025-03-26"`
///    (the latest version rmcp 0.13 supports), allowing rmcp to
///    accept the request.
async fn ensure_accept_header(
    axum::extract::State(pending_channel): axum::extract::State<Arc<Mutex<Option<String>>>>,
    request: Request,
    next: Next,
) -> Response {
    // Store the channel_id from the URL so the factory can pick it up
    // when rmcp creates a new session.
    if let Some(ch) = extract_channel_id(request.uri()) {
        if let Ok(mut guard) = pending_channel.lock() {
            *guard = Some(ch);
        }
    }
    let method = request.method().clone();
    let uri = request.uri().clone();
    let accept_before = request
        .headers()
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<none>")
        .to_owned();
    let content_type = request
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<none>")
        .to_owned();
    let session_id = request
        .headers()
        .get("Mcp-Session-Id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<none>")
        .to_owned();

    // Read the request body so we can inspect and potentially rewrite it.
    let (parts, body) = request.into_parts();
    let body_bytes = match axum::body::to_bytes(body, 64 * 1024).await {
        Ok(b) => b,
        Err(err) => {
            debug!(%method, %uri, %err, "failed to read request body");
            return (StatusCode::BAD_REQUEST, "failed to read body").into_response();
        }
    };

    let body_preview = String::from_utf8_lossy(&body_bytes[..body_bytes.len().min(512)]);
    debug!(
        %method, %uri,
        accept = %accept_before,
        content_type = %content_type,
        session_id = %session_id,
        body = %body_preview,
        "mcp request received (pre-middleware)"
    );

    // Sanitize Initialize requests for rmcp 0.13 compatibility.
    let final_body = sanitize_initialize_body(&body_bytes);

    // Reconstruct the request with the (possibly rewritten) body.
    let mut request = Request::from_parts(parts, axum::body::Body::from(final_body));

    // Fix up the Accept header.
    if let Some(accept) = request.headers().get(axum::http::header::ACCEPT) {
        if let Ok(val) = accept.to_str() {
            let has_json = val.contains("application/json");
            let has_sse = val.contains("text/event-stream");
            if has_json && !has_sse {
                let new_val = format!("{val}, text/event-stream");
                if let Ok(hv) = new_val.parse() {
                    request.headers_mut().insert(axum::http::header::ACCEPT, hv);
                }
            } else if has_sse && !has_json {
                let new_val = format!("application/json, {val}");
                if let Ok(hv) = new_val.parse() {
                    request.headers_mut().insert(axum::http::header::ACCEPT, hv);
                }
            } else if !has_json && !has_sse {
                if let Ok(hv) = "application/json, text/event-stream".parse() {
                    request.headers_mut().insert(axum::http::header::ACCEPT, hv);
                }
            }
        }
    } else {
        // No Accept header at all — add the required one.
        if let Ok(hv) = "application/json, text/event-stream".parse() {
            request.headers_mut().insert(axum::http::header::ACCEPT, hv);
        }
    }

    let response = next.run(request).await;

    // rmcp returns 401 for unknown/stale session IDs.  VS Code treats
    // HTTP 401 as "needs OAuth" and opens a browser window.  Convert
    // to 400 Bad Request so VS Code shows a connection error instead,
    // prompting the user to restart the MCP connection (which sends a
    // fresh Initialize without a session ID).
    let final_response = if response.status() == StatusCode::UNAUTHORIZED {
        let had_session = session_id != "<none>";
        debug!(
            %method, %uri,
            had_session,
            "converting 401 → 400 to prevent OAuth dance"
        );
        (
            StatusCode::BAD_REQUEST,
            "session expired or unknown — restart the MCP connection",
        )
            .into_response()
    } else {
        response
    };

    debug!(
        %method, %uri,
        status = %final_response.status(),
        "mcp response"
    );
    final_response
}

/// The protocol version that rmcp 0.13 reports as LATEST.
const RMCP_LATEST_PROTOCOL_VERSION: &str = "2025-03-26";

/// Known capability fields that rmcp 0.13's `ClientCapabilities` can
/// deserialize without type mismatches.  VS Code 1.109+ sends `tasks`,
/// `elicitation`, and `sampling` with object shapes that differ from
/// what rmcp 0.13 expects (e.g. `"list": {}` instead of `"list": true`).
/// Rather than attempting field-level fixups, we keep only the fields
/// our server actually uses and that deserialize cleanly.
const SAFE_CAPABILITY_FIELDS: &[&str] = &["experimental", "roots"];

/// Rewrite an Initialize request body to be compatible with rmcp 0.13.
///
/// If the body is not an Initialize request or not valid JSON, it is
/// returned unchanged.
fn sanitize_initialize_body(raw: &[u8]) -> Vec<u8> {
    let Ok(mut msg) = serde_json::from_slice::<Value>(raw) else {
        return raw.to_vec();
    };

    // Only rewrite `initialize` requests.
    let is_initialize = msg
        .get("method")
        .and_then(Value::as_str)
        .is_some_and(|m| m == "initialize");
    if !is_initialize {
        return raw.to_vec();
    }

    let Some(params) = msg.get_mut("params").and_then(Value::as_object_mut) else {
        return raw.to_vec();
    };

    // Downgrade protocolVersion if rmcp 0.13 doesn't know it.
    if let Some(pv) = params.get("protocolVersion").and_then(Value::as_str) {
        if pv != "2024-11-05" && pv != "2025-03-26" && pv != "2025-06-18" {
            debug!(
                original = %pv,
                rewritten = RMCP_LATEST_PROTOCOL_VERSION,
                "downgrading protocolVersion for rmcp 0.13 compat"
            );
            params.insert(
                "protocolVersion".to_owned(),
                Value::String(RMCP_LATEST_PROTOCOL_VERSION.to_owned()),
            );
        }
    }

    // Strip capability fields that rmcp 0.13 cannot deserialize.
    if let Some(caps) = params
        .get_mut("capabilities")
        .and_then(Value::as_object_mut)
    {
        let unknown_keys: Vec<String> = caps
            .keys()
            .filter(|k| !SAFE_CAPABILITY_FIELDS.contains(&k.as_str()))
            .cloned()
            .collect();
        for key in &unknown_keys {
            debug!(field = %key, "stripping capability field for rmcp 0.13 compat");
            caps.remove(key);
        }
    }

    // Re-serialize.  If serialization somehow fails, fall back to
    // the original bytes.
    serde_json::to_vec(&msg).unwrap_or_else(|_| raw.to_vec())
}

/// Handler for the legacy `/sse` endpoint.
///
/// Returns `410 Gone` so that clients using the old SSE transport know
/// they must upgrade to the `/mcp` Streamable-HTTP endpoint.
async fn sse_gone() -> StatusCode {
    StatusCode::GONE
}

/// Extract `channel_id` from a URI query string.
///
/// Returns `None` when the parameter is absent or empty.
fn extract_channel_id(uri: &axum::http::Uri) -> Option<String> {
    uri.query().and_then(|q| {
        q.split('&')
            .filter_map(|pair| pair.split_once('='))
            .find(|(k, _)| *k == "channel_id")
            .map(|(_, v)| v.to_owned())
            .filter(|v| !v.is_empty())
    })
}

/// Diagnostic middleware that logs every inbound HTTP request.
///
/// Applied to the outer router to capture requests to all endpoints,
/// including OAuth stubs and the MCP endpoint.
async fn log_all_requests(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let accept = request
        .headers()
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<none>")
        .to_owned();
    let content_type = request
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<none>")
        .to_owned();
    let auth = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map_or("<none>", |v| if v.len() > 20 { &v[..20] } else { v })
        .to_owned();

    debug!(
        %method, %uri,
        accept = %accept,
        content_type = %content_type,
        authorization = %auth,
        "inbound request"
    );

    let response = next.run(request).await;
    debug!(
        %method, %uri,
        status = %response.status(),
        "outbound response"
    );
    response
}

/// Start the HTTP/Streamable-HTTP MCP transport on `config.http_port`.
///
/// Each MCP connection creates a fresh [`IntercomServer`] sharing the
/// same [`AppState`].  Channel IDs are resolved via the `channel_id` query
/// parameter on the `/mcp` endpoint.
///
/// The legacy `/sse` endpoint responds with `410 Gone`.
///
/// # Errors
///
/// Returns `AppError::Config` if the server fails to bind.
pub async fn serve_http(state: Arc<AppState>, ct: CancellationToken) -> Result<()> {
    let port = state.config.http_port;
    let bind = SocketAddr::from(([127, 0, 0, 1], port));

    let config = StreamableHttpServerConfig {
        cancellation_token: ct.child_token(),
        // Disable SSE keep-alive comments — VS Code's Streamable-HTTP
        // parser cannot handle SSE comments (`:` lines) or empty `data:`
        // priming events (SEP-1699), logging "Failed to parse message"
        // warnings every 15 s.  Safe for localhost where no proxy/NAT
        // would drop idle connections.
        sse_keep_alive: None,
        ..Default::default()
    };

    let session_manager = Arc::new(LocalSessionManager::default());

    // Shared slot for passing channel_id from the middleware into the
    // rmcp factory closure.  The middleware writes the channel_id
    // extracted from the URL query string; the factory `.take()`s it
    // when rmcp creates a new session (Initialize without session ID).
    let pending_channel: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    // Each inbound MCP connection gets its own IntercomServer instance.
    let state_for_factory = Arc::clone(&state);
    let pending_for_factory = Arc::clone(&pending_channel);
    let service = StreamableHttpService::new(
        move || {
            let channel = pending_for_factory
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            debug!(
                channel_id = channel.as_deref().unwrap_or("<none>"),
                "creating IntercomServer for new MCP session"
            );
            Ok(IntercomServer::with_channel_override(
                Arc::clone(&state_for_factory),
                channel,
            ))
        },
        session_manager,
        config,
    );

    // Wrap the MCP endpoint with middleware that fixes missing Accept
    // headers so that VS Code's initial POST probe succeeds without
    // falling back to legacy SSE transport and the OAuth dance.
    let mcp_service =
        axum::Router::new()
            .fallback_service(service)
            .layer(middleware::from_fn_with_state(
                pending_channel,
                ensure_accept_header,
            ));

    let router = axum::Router::new()
        .nest("/mcp", mcp_service)
        .route("/health", get(health))
        .route("/sse", get(sse_gone))
        .layer(middleware::from_fn(log_all_requests));

    info!("registered routes: /mcp, /health, /sse");

    // Serve HTTP via axum.
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|err| AppError::Config(format!("failed to bind HTTP on {bind}: {err}")))?;

    info!(%bind, "starting HTTP/Streamable-HTTP MCP transport");

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            ct.cancelled().await;
        })
        .await
        .map_err(|err| AppError::Config(format!("HTTP server error: {err}")))?;

    info!("HTTP/Streamable-HTTP MCP transport shut down");
    Ok(())
}

/// Deprecated alias for [`serve_http`].
///
/// Retained for backwards compatibility with call sites that used the
/// old SSE-based function name. New code should call [`serve_http`].
///
/// # Errors
///
/// Returns `AppError::Config` if the server fails to bind. See [`serve_http`].
#[deprecated(since = "0.2.0", note = "Use `serve_http` instead")]
pub async fn serve_sse(state: Arc<AppState>, ct: CancellationToken) -> Result<()> {
    serve_http(state, ct).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::expect_used)]
    fn parse_uri(s: &str) -> axum::http::Uri {
        s.parse().expect("valid URI")
    }

    #[test]
    fn channel_id_present_returns_value() {
        let uri = parse_uri("/sse?channel_id=C_WORKSPACE");
        assert_eq!(extract_channel_id(&uri), Some("C_WORKSPACE".to_owned()));
    }

    #[test]
    fn missing_channel_id_returns_none() {
        let uri = parse_uri("/sse");
        assert_eq!(extract_channel_id(&uri), None);
    }

    #[test]
    fn empty_channel_id_returns_none() {
        let uri = parse_uri("/sse?channel_id=");
        assert_eq!(extract_channel_id(&uri), None);
    }

    #[test]
    fn multiple_channel_id_params_first_wins() {
        let uri = parse_uri("/sse?channel_id=C_FIRST&channel_id=C_SECOND");
        assert_eq!(extract_channel_id(&uri), Some("C_FIRST".to_owned()));
    }

    #[test]
    fn channel_id_with_no_equals_returns_none() {
        let uri = parse_uri("/sse?channel_id");
        assert_eq!(extract_channel_id(&uri), None);
    }

    #[test]
    fn channel_id_among_other_params() {
        let uri = parse_uri("/sse?foo=bar&channel_id=C_TARGET&baz=qux");
        assert_eq!(extract_channel_id(&uri), Some("C_TARGET".to_owned()));
    }

    #[test]
    fn url_encoded_channel_id_passes_through_raw() {
        // Slack channel IDs are alphanumeric (C[A-Z0-9]+), so URL encoding
        // is not a practical concern. The function intentionally does NOT
        // URL-decode values, keeping the implementation simple.
        let uri = parse_uri("/sse?channel_id=C_TEST%20SPACE");
        assert_eq!(extract_channel_id(&uri), Some("C_TEST%20SPACE".to_owned()));
    }

    // ── sanitize_initialize_body tests ───────────────────

    #[test]
    fn sanitize_non_json_returns_original_bytes() {
        let raw = b"not valid json at all";
        let result = sanitize_initialize_body(raw);
        assert_eq!(result, raw);
    }

    #[test]
    fn sanitize_non_initialize_method_returns_original() {
        let raw = br#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#;
        let result = sanitize_initialize_body(raw);
        // Should return the original bytes untouched.
        assert_eq!(result, raw.to_vec());
    }

    #[test]
    fn sanitize_known_protocol_version_unchanged() {
        let raw = br#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2025-03-26","capabilities":{}}}"#;
        let result = sanitize_initialize_body(raw);
        let parsed: Value = serde_json::from_slice(&result).expect("valid json");
        assert_eq!(
            parsed["params"]["protocolVersion"].as_str(),
            Some("2025-03-26")
        );
    }

    #[test]
    fn sanitize_unknown_protocol_version_downgraded() {
        let raw = br#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2025-11-25","capabilities":{}}}"#;
        let result = sanitize_initialize_body(raw);
        let parsed: Value = serde_json::from_slice(&result).expect("valid json");
        assert_eq!(
            parsed["params"]["protocolVersion"].as_str(),
            Some(RMCP_LATEST_PROTOCOL_VERSION)
        );
    }

    #[test]
    fn sanitize_strips_unknown_capability_fields() {
        let raw = br#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2025-03-26","capabilities":{"experimental":{},"roots":{"listChanged":true},"sampling":{},"elicitation":{},"tasks":{"list":{}}}}}"#;
        let result = sanitize_initialize_body(raw);
        let parsed: Value = serde_json::from_slice(&result).expect("valid json");
        let caps = parsed["params"]["capabilities"].as_object().expect("obj");
        // Only safe fields should remain.
        assert!(caps.contains_key("experimental"));
        assert!(caps.contains_key("roots"));
        assert!(!caps.contains_key("sampling"));
        assert!(!caps.contains_key("elicitation"));
        assert!(!caps.contains_key("tasks"));
    }

    #[test]
    fn sanitize_empty_capabilities_no_crash() {
        let raw = br#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2025-03-26","capabilities":{}}}"#;
        let result = sanitize_initialize_body(raw);
        let parsed: Value = serde_json::from_slice(&result).expect("valid json");
        let caps = parsed["params"]["capabilities"].as_object().expect("obj");
        assert!(caps.is_empty());
    }

    #[test]
    fn sanitize_missing_params_returns_original() {
        let raw = br#"{"jsonrpc":"2.0","method":"initialize","id":1}"#;
        let result = sanitize_initialize_body(raw);
        // No params → returned unchanged.
        assert_eq!(result, raw.to_vec());
    }

    #[test]
    fn sanitize_preserves_2024_protocol_version() {
        let raw = br#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2024-11-05","capabilities":{}}}"#;
        let result = sanitize_initialize_body(raw);
        let parsed: Value = serde_json::from_slice(&result).expect("valid json");
        assert_eq!(
            parsed["params"]["protocolVersion"].as_str(),
            Some("2024-11-05")
        );
    }

    #[test]
    fn sanitize_preserves_2025_06_18_protocol_version() {
        let raw = br#"{"jsonrpc":"2.0","method":"initialize","id":1,"params":{"protocolVersion":"2025-06-18","capabilities":{}}}"#;
        let result = sanitize_initialize_body(raw);
        let parsed: Value = serde_json::from_slice(&result).expect("valid json");
        assert_eq!(
            parsed["params"]["protocolVersion"].as_str(),
            Some("2025-06-18")
        );
    }
}
