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
use tracing::info;

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
            let channel_override = inbox_for_factory.lock().expect("inbox lock").take();
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
                let _permit = sem.acquire().await.expect("semaphore closed");
                let channel_id = extract_channel_id(request.uri());
                *inbox.lock().expect("inbox lock") = channel_id;
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
