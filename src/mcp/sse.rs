//! HTTP/SSE transport for multi-agent connections.
//!
//! Mounts an [`SseServer`] behind an axum router so that remote agents
//! can connect via HTTP with Server-Sent Events streaming.
//!
//! The SSE endpoint accepts an optional `channel_id` query parameter
//! (e.g. `/sse?channel_id=C_WORKSPACE_CHANNEL`) so that each connected
//! workspace can target a different Slack channel.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::Request;
use axum::middleware::{self, Next};
use axum::response::Response;
use rmcp::transport::sse_server::{SseServer, SseServerConfig};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::handler::{AgentRemServer, AppState};
use crate::{AppError, Result};

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

/// Start the HTTP/SSE MCP transport on `config.http_port`.
///
/// Each SSE connection creates a fresh [`AgentRemServer`] sharing the
/// same [`AppState`].  When the client connects with a `channel_id`
/// query parameter the per-session Slack channel is overridden.
///
/// # Errors
///
/// Returns `AppError::Config` if the server fails to bind.
pub async fn serve_sse(state: Arc<AppState>, ct: CancellationToken) -> Result<()> {
    let port = state.config.http_port;
    let bind = SocketAddr::from(([127, 0, 0, 1], port));

    let config = SseServerConfig {
        bind,
        sse_path: "/sse".into(),
        post_path: "/message".into(),
        ct: ct.clone(),
        sse_keep_alive: None,
    };

    let (sse_server, router) = SseServer::new(config);

    // Shared inbox: the middleware writes the channel_id extracted from
    // the query string; the factory closure reads it when creating the
    // per-session AgentRemServer.  A semaphore serialises SSE connection
    // establishment so the inbox value is never clobbered by a concurrent
    // connection.
    let channel_inbox: Arc<std::sync::Mutex<Option<String>>> =
        Arc::new(std::sync::Mutex::new(None));
    let connection_semaphore = Arc::new(Semaphore::new(1));

    // Each inbound SSE connection gets its own AgentRemServer instance.
    let inbox_for_factory = Arc::clone(&channel_inbox);
    let server_ct = {
        let state = Arc::clone(&state);
        sse_server.with_service(move || {
            let channel_override = inbox_for_factory
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            if let Some(ref ch) = channel_override {
                info!(channel_id = %ch, "SSE session with per-workspace channel override");
            }
            AgentRemServer::with_channel_override(Arc::clone(&state), channel_override)
        })
    };

    // Middleware: extract `channel_id` from the query string on `/sse`
    // requests and store it in the inbox while holding the semaphore.
    let inbox_for_mw = Arc::clone(&channel_inbox);
    let sem_for_mw = Arc::clone(&connection_semaphore);
    let router = router.layer(middleware::from_fn(move |request: Request, next: Next| {
        let inbox = Arc::clone(&inbox_for_mw);
        let sem = Arc::clone(&sem_for_mw);
        async move {
            let is_sse = request.uri().path() == "/sse";
            if is_sse {
                // Serialise so the inbox value is consumed by exactly
                // the factory call that corresponds to this request.
                let Ok(_permit) = sem.acquire().await else {
                    warn!("connection semaphore closed; skipping channel override");
                    return next.run(request).await;
                };
                let channel_id = extract_channel_id(request.uri());
                *inbox
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = channel_id;
                let response: Response = next.run(request).await;
                // _permit drops here after the factory has consumed the inbox
                response
            } else {
                next.run(request).await
            }
        }
    }));

    // Serve HTTP via axum.
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|err| AppError::Config(format!("failed to bind SSE on {bind}: {err}")))?;

    info!(%bind, "starting HTTP/SSE MCP transport");

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            ct.cancelled().await;
            server_ct.cancel();
        })
        .await
        .map_err(|err| AppError::Config(format!("SSE server error: {err}")))?;

    info!("HTTP/SSE MCP transport shut down");
    Ok(())
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
