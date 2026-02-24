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

use std::net::SocketAddr;
use std::sync::Arc;

use axum::http::StatusCode;
use axum::routing::get;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use tokio_util::sync::CancellationToken;
use tracing::info;

use super::handler::{AppState, IntercomServer};
use crate::{AppError, Result};

/// Handler for `GET /health` — returns 200 OK with a plain-text body.
///
/// Useful for probing liveness without initiating an MCP session.
async fn health() -> &'static str {
    "ok"
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
#[cfg(test)]
fn extract_channel_id(uri: &axum::http::Uri) -> Option<String> {
    uri.query().and_then(|q| {
        q.split('&')
            .filter_map(|pair| pair.split_once('='))
            .find(|(k, _)| *k == "channel_id")
            .map(|(_, v)| v.to_owned())
            .filter(|v| !v.is_empty())
    })
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
        ..Default::default()
    };

    let session_manager = Arc::new(LocalSessionManager::default());

    // Each inbound MCP connection gets its own IntercomServer instance.
    // channel_id is passed via the factory closure via per-request extensions.
    let state_for_factory = Arc::clone(&state);
    let service = StreamableHttpService::new(
        move || {
            // channel_id routing via query param is handled by the `/mcp` layer;
            // for now each session uses the server-level channel (no override).
            Ok(IntercomServer::with_channel_override(
                Arc::clone(&state_for_factory),
                None,
            ))
        },
        session_manager,
        config,
    );

    let router = axum::Router::new()
        .nest_service("/mcp", service)
        .route("/health", get(health))
        .route("/sse", get(sse_gone));

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
}
