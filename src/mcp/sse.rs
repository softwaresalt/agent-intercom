//! HTTP/SSE transport for multi-agent connections.
//!
//! Mounts an [`SseServer`] behind an axum router so that remote agents
//! can connect via HTTP with Server-Sent Events streaming.

use std::net::SocketAddr;
use std::sync::Arc;

use rmcp::transport::sse_server::{SseServer, SseServerConfig};
use tokio_util::sync::CancellationToken;
use tracing::info;

use super::handler::{AgentRemServer, AppState};
use crate::{AppError, Result};

/// Start the HTTP/SSE MCP transport on `config.http_port`.
///
/// Each SSE connection creates a fresh [`AgentRemServer`] sharing the
/// same [`AppState`].
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

    // Each inbound SSE connection gets its own AgentRemServer instance.
    let server_ct = {
        let state = Arc::clone(&state);
        sse_server.with_service(move || AgentRemServer::new(Arc::clone(&state)))
    };

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
