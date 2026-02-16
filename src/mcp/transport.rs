//! Stdio transport setup for the primary agent connection.
//!
//! Wires [`AgentRemServer`] to stdin/stdout for direct invocation
//! by agentic IDEs (Claude Code, GitHub Copilot CLI, Cursor, VS Code).

use std::sync::Arc;

use rmcp::service::ServiceExt;
use rmcp::transport::io::stdio;
use tokio_util::sync::CancellationToken;
use tracing::info;

use super::handler::{AgentRemServer, AppState};
use crate::{AppError, Result};

/// Serve the MCP server over stdio until the cancellation token fires.
///
/// # Errors
///
/// Returns `AppError::Config` if the transport fails to initialize.
pub async fn serve_stdio(state: Arc<AppState>, ct: CancellationToken) -> Result<()> {
    let server = AgentRemServer::new(state);
    let transport = stdio();

    info!("starting stdio MCP transport");
    let service = server
        .serve_with_ct(transport, ct)
        .await
        .map_err(|err| AppError::Config(format!("stdio transport failed: {err}")))?;

    service
        .waiting()
        .await
        .map_err(|err| AppError::Config(format!("stdio service error: {err}")))?;

    info!("stdio MCP transport shut down");
    Ok(())
}
