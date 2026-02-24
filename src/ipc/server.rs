//! Local IPC server for `agent-intercom-ctl` commands (T087).
//!
//! Listens on a named pipe (Windows) or Unix domain socket (Linux/macOS)
//! using the `interprocess` crate. Accepts line-delimited JSON commands
//! from `agent-intercom-ctl` and routes them to the appropriate handler.
//!
//! ## Protocol
//!
//! Request (one JSON object per line):
//! ```json
//! {"command": "list"}
//! {"command": "approve", "id": "req-123"}
//! {"command": "reject", "id": "req-123", "reason": "too risky"}
//! {"command": "resume", "instruction": "deploy to staging"}
//! {"command": "mode", "mode": "local"}
//! ```
//!
//! Response (one JSON object per line):
//! ```json
//! {"ok": true, "data": { ... } }
//! {"ok": false, "error": "not found"}
//! ```

use std::sync::Arc;

use interprocess::local_socket::{tokio::prelude::*, GenericNamespaced, ListenerOptions};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio_util::sync::CancellationToken;
use tracing::{info, info_span, warn, Instrument};

use crate::mcp::handler::{AppState, ApprovalResponse, WaitResponse};
use crate::models::session::SessionMode;
use crate::persistence::approval_repo::ApprovalRepo;
use crate::persistence::session_repo::SessionRepo;
use crate::{AppError, Result};

/// Inbound IPC request from `agent-intercom-ctl`.
#[derive(Debug, Deserialize)]
struct IpcRequest {
    /// Command verb.
    command: String,
    /// Entity identifier (for `approve`, `reject`).
    id: Option<String>,
    /// Rejection reason or resume instruction text.
    reason: Option<String>,
    /// Resume instruction text.
    instruction: Option<String>,
    /// Target mode (for `mode` command).
    mode: Option<String>,
    /// Shared-secret authentication token.
    auth_token: Option<String>,
}

/// Outbound IPC response to `agent-intercom-ctl`.
#[derive(Debug, Serialize)]
struct IpcResponse {
    /// Whether the command succeeded.
    ok: bool,
    /// Payload on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
    /// Error message on failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl IpcResponse {
    fn success(data: serde_json::Value) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

/// Spawn the IPC server task.
///
/// # Errors
///
/// Returns `AppError::Ipc` if the listener cannot be created.
pub fn spawn_ipc_server(
    state: Arc<AppState>,
    ct: CancellationToken,
) -> Result<tokio::task::JoinHandle<()>> {
    let name = state.config.ipc_name.clone();

    let listener_name = name
        .clone()
        .to_ns_name::<GenericNamespaced>()
        .map_err(|err| AppError::Ipc(format!("invalid ipc socket name '{name}': {err}")))?;

    let listener = ListenerOptions::new()
        .name(listener_name)
        .create_tokio()
        .map_err(|err| AppError::Ipc(format!("failed to create ipc listener: {err}")))?;

    info!(ipc_name = %name, "IPC server listening");

    let handle = tokio::spawn(async move {
        let span = info_span!("ipc_server", name = %name);
        async move {
            loop {
                tokio::select! {
                    () = ct.cancelled() => {
                        info!("IPC server shutting down");
                        break;
                    }
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok(stream) => {
                                let state = Arc::clone(&state);
                                tokio::spawn(handle_connection(stream, state));
                            }
                            Err(err) => {
                                warn!(%err, "IPC accept failed");
                            }
                        }
                    }
                }
            }
        }
        .instrument(span)
        .await;
    });

    Ok(handle)
}

/// Handle a single IPC client connection.
async fn handle_connection(
    stream: interprocess::local_socket::tokio::Stream,
    state: Arc<AppState>,
) {
    let span = info_span!("ipc_conn");
    async move {
        let (reader, mut writer) = stream.split();
        let mut buf_reader = BufReader::new(reader);
        let mut line = String::new();

        loop {
            line.clear();
            match buf_reader.read_line(&mut line).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    let response = match serde_json::from_str::<IpcRequest>(trimmed) {
                        Ok(request) => dispatch_command(&request, &state).await,
                        Err(err) => IpcResponse::error(format!("invalid json: {err}")),
                    };

                    let mut response_line = serde_json::to_string(&response).unwrap_or_else(|_| {
                        r#"{"ok":false,"error":"serialization failed"}"#.to_owned()
                    });
                    response_line.push('\n');

                    if let Err(err) = writer.write_all(response_line.as_bytes()).await {
                        warn!(%err, "failed to write ipc response");
                        break;
                    }
                }
                Err(err) => {
                    warn!(%err, "ipc read error");
                    break;
                }
            }
        }

        info!("IPC connection closed");
    }
    .instrument(span)
    .await;
}

/// Route an IPC command to the appropriate handler.
async fn dispatch_command(request: &IpcRequest, state: &Arc<AppState>) -> IpcResponse {
    let span = info_span!("ipc_command", command = %request.command);
    let _guard = span.enter();

    // Validate shared-secret auth token when configured.
    if let Some(ref expected) = state.ipc_auth_token {
        match request.auth_token {
            Some(ref provided) if provided == expected => {}
            _ => {
                warn!(command = %request.command, "IPC request rejected: invalid auth token");
                return IpcResponse::error("unauthorized");
            }
        }
    }

    match request.command.as_str() {
        "list" => handle_list(state).await,
        "approve" => handle_approve(request, state).await,
        "reject" => handle_reject(request, state).await,
        "resume" => handle_resume(request, state).await,
        "mode" => handle_mode(request, state).await,
        other => IpcResponse::error(format!("unknown command: {other}")),
    }
}

/// List active sessions.
async fn handle_list(state: &Arc<AppState>) -> IpcResponse {
    let session_repo = SessionRepo::new(Arc::clone(&state.db));
    match session_repo.list_active().await {
        Ok(sessions) => {
            let items: Vec<serde_json::Value> = sessions
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "session_id": s.id,
                        "status": format!("{:?}", s.status).to_lowercase(),
                        "mode": format!("{:?}", s.mode).to_lowercase(),
                        "workspace_root": s.workspace_root,
                        "last_tool": s.last_tool,
                        "updated_at": s.updated_at.to_rfc3339(),
                    })
                })
                .collect();
            IpcResponse::success(serde_json::json!({ "sessions": items }))
        }
        Err(err) => IpcResponse::error(format!("failed to list sessions: {err}")),
    }
}

/// Approve a pending approval request via IPC.
async fn handle_approve(request: &IpcRequest, state: &Arc<AppState>) -> IpcResponse {
    let Some(ref id) = request.id else {
        return IpcResponse::error("missing required 'id' field");
    };

    // Update DB status.
    let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));
    if let Err(err) = approval_repo
        .update_status(id, crate::models::approval::ApprovalStatus::Approved)
        .await
    {
        return IpcResponse::error(format!("failed to approve: {err}"));
    }

    // Resolve the pending oneshot.
    {
        let mut pending = state.pending_approvals.lock().await;
        if let Some(tx) = pending.remove(id.as_str()) {
            let response = ApprovalResponse {
                status: "approved".to_owned(),
                reason: None,
            };
            let _ = tx.send(response);
        }
    }

    info!(request_id = %id, "approved via IPC");
    IpcResponse::success(serde_json::json!({ "request_id": id, "status": "approved" }))
}

/// Reject a pending approval request via IPC.
async fn handle_reject(request: &IpcRequest, state: &Arc<AppState>) -> IpcResponse {
    let Some(ref id) = request.id else {
        return IpcResponse::error("missing required 'id' field");
    };

    let reason = request
        .reason
        .clone()
        .unwrap_or_else(|| "rejected via local CLI".to_owned());

    // Update DB status.
    let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));
    if let Err(err) = approval_repo
        .update_status(id, crate::models::approval::ApprovalStatus::Rejected)
        .await
    {
        return IpcResponse::error(format!("failed to reject: {err}"));
    }

    // Resolve the pending oneshot.
    {
        let mut pending = state.pending_approvals.lock().await;
        if let Some(tx) = pending.remove(id.as_str()) {
            let response = ApprovalResponse {
                status: "rejected".to_owned(),
                reason: Some(reason.clone()),
            };
            let _ = tx.send(response);
        }
    }

    info!(request_id = %id, "rejected via IPC");
    IpcResponse::success(serde_json::json!({ "request_id": id, "status": "rejected" }))
}

/// Resume a waiting agent via IPC.
///
/// When `request.id` contains a session ID, that specific session is resumed.
/// Otherwise the first pending wait is used (for single-session scenarios).
async fn handle_resume(request: &IpcRequest, state: &Arc<AppState>) -> IpcResponse {
    let instruction = request.instruction.clone();

    // Prefer explicit session_id from the request when available.
    let session_id = if let Some(ref sid) = request.id {
        let pending = state.pending_waits.lock().await;
        if pending.contains_key(sid) {
            Some(sid.clone())
        } else {
            return IpcResponse::error(format!("session {sid} is not waiting"));
        }
    } else {
        let pending = state.pending_waits.lock().await;
        pending.keys().next().cloned()
    };

    let Some(session_id) = session_id else {
        return IpcResponse::error("no agent currently waiting for instruction");
    };

    // Resolve the oneshot.
    {
        let mut pending = state.pending_waits.lock().await;
        if let Some(tx) = pending.remove(&session_id) {
            let response = WaitResponse {
                status: "resumed".to_owned(),
                instruction: instruction.clone(),
            };
            let _ = tx.send(response);
        }
    }

    info!(session_id = %session_id, "agent resumed via IPC");
    IpcResponse::success(serde_json::json!({ "session_id": session_id, "status": "resumed" }))
}

/// Change operational mode via IPC.
async fn handle_mode(request: &IpcRequest, state: &Arc<AppState>) -> IpcResponse {
    let Some(ref mode_str) = request.mode else {
        return IpcResponse::error("missing required 'mode' field");
    };

    let mode = match mode_str.as_str() {
        "remote" => SessionMode::Remote,
        "local" => SessionMode::Local,
        "hybrid" => SessionMode::Hybrid,
        other => return IpcResponse::error(format!("invalid mode: {other}")),
    };

    let session_repo = SessionRepo::new(Arc::clone(&state.db));
    let sessions = match session_repo.list_active().await {
        Ok(s) => s,
        Err(err) => return IpcResponse::error(format!("failed to query sessions: {err}")),
    };

    let Some(session) = sessions.into_iter().next() else {
        return IpcResponse::error("no active session found");
    };

    let previous_mode = session.mode;

    if let Err(err) = session_repo.update_mode(&session.id, mode).await {
        return IpcResponse::error(format!("failed to update mode: {err}"));
    }

    info!(
        session_id = %session.id,
        ?previous_mode,
        current_mode = ?mode,
        "mode changed via IPC"
    );

    IpcResponse::success(serde_json::json!({
        "previous_mode": format!("{previous_mode:?}").to_lowercase(),
        "current_mode": mode_str,
    }))
}
