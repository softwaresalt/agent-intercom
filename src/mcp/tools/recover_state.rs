//! `recover_state` MCP tool handler (T080, T083).
//!
//! Retrieves the last known state from persistent storage. Called by the
//! agent on startup to check for interrupted sessions or pending requests.

use std::sync::Arc;

use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::CallToolResult;
use tracing::{info, info_span, Instrument};

use crate::mcp::handler::{AppState, IntercomServer};
use crate::models::session::Session;
use crate::persistence::approval_repo::ApprovalRepo;
use crate::persistence::checkpoint_repo::CheckpointRepo;
use crate::persistence::prompt_repo::PromptRepo;
use crate::persistence::session_repo::SessionRepo;

/// Input parameters per mcp-tools.json contract.
#[derive(Debug, serde::Deserialize)]
struct RecoverStateInput {
    /// Optional session to recover. When omitted, finds the most recently
    /// active/interrupted session.
    session_id: Option<String>,
}

/// Handle the `recover_state` tool call.
///
/// # Errors
///
/// Returns `rmcp::ErrorData` on persistence failures.
#[allow(clippy::too_many_lines)] // Recovery logic is inherently multi-step.
pub async fn handle(
    context: ToolCallContext<'_, IntercomServer>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let state = Arc::clone(context.service.state());
    let args: serde_json::Map<String, serde_json::Value> = context.arguments.unwrap_or_default();

    let input: RecoverStateInput = serde_json::from_value(serde_json::Value::Object(args))
        .map_err(|err| {
            rmcp::ErrorData::invalid_params(
                format!("invalid recover_state parameters: {err}"),
                None,
            )
        })?;

    let span = info_span!(
        "recover_state",
        session_id = input.session_id.as_deref().unwrap_or("auto"),
    );

    async move {
        let session_repo = SessionRepo::new(Arc::clone(&state.db));

        // ── Resolve session ──────────────────────────────────
        let session = resolve_session(&session_repo, input.session_id.as_deref()).await?;

        let Some(session) = session else {
            info!("no interrupted session found — clean state");
            return json_result(serde_json::json!({ "status": "clean" }));
        };

        // ── Collect pending data and build response ──────────
        let response = build_recovered_response(&state, &session).await?;

        let pending_count = response
            .get("pending_requests")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len);

        info!(
            session_id = %session.id,
            pending_count,
            "state recovered"
        );

        json_result(response)
    }
    .instrument(span)
    .await
}

/// Resolve the target session for recovery.
async fn resolve_session(
    repo: &SessionRepo,
    session_id: Option<&str>,
) -> Result<Option<Session>, rmcp::ErrorData> {
    if let Some(sid) = session_id {
        repo.get_by_id(sid).await.map_err(|err| {
            rmcp::ErrorData::internal_error(format!("failed to query session {sid}: {err}"), None)
        })
    } else {
        repo.get_most_recent_interrupted().await.map_err(|err| {
            rmcp::ErrorData::internal_error(
                format!("failed to query interrupted sessions: {err}"),
                None,
            )
        })
    }
}

/// Collect pending approvals, prompts, checkpoints, and progress snapshot
/// into the `recovered` response JSON.
async fn build_recovered_response(
    state: &AppState,
    session: &Session,
) -> Result<serde_json::Value, rmcp::ErrorData> {
    let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));
    let prompt_repo = PromptRepo::new(Arc::clone(&state.db));
    let checkpoint_repo = CheckpointRepo::new(Arc::clone(&state.db));

    // ── Pending approval requests ────────────────────────
    let pending_approval = approval_repo
        .get_pending_for_session(&session.id)
        .await
        .map_err(|err| {
            rmcp::ErrorData::internal_error(
                format!("failed to query pending approvals: {err}"),
                None,
            )
        })?;

    // ── Pending prompts ──────────────────────────────────
    let pending_prompt = prompt_repo
        .get_pending_for_session(&session.id)
        .await
        .map_err(|err| {
            rmcp::ErrorData::internal_error(format!("failed to query pending prompts: {err}"), None)
        })?;

    // ── Build pending_requests array ─────────────────────
    let mut pending_requests = Vec::new();
    if let Some(ref approval) = pending_approval {
        pending_requests.push(serde_json::json!({
            "request_id": approval.id,
            "type": "approval",
            "title": approval.title,
            "created_at": approval.created_at.to_rfc3339(),
        }));
    }
    if let Some(ref prompt) = pending_prompt {
        pending_requests.push(serde_json::json!({
            "request_id": prompt.id,
            "type": "prompt",
            "title": prompt.prompt_text,
            "created_at": prompt.created_at.to_rfc3339(),
        }));
    }

    // ── Last checkpoint ──────────────────────────────────
    let checkpoints = checkpoint_repo
        .list_for_session(&session.id)
        .await
        .map_err(|err| {
            rmcp::ErrorData::internal_error(format!("failed to query checkpoints: {err}"), None)
        })?;
    let last_checkpoint = checkpoints.first().map(|cp| {
        serde_json::json!({
            "checkpoint_id": cp.id,
            "label": cp.label,
            "created_at": cp.created_at.to_rfc3339(),
        })
    });

    // ── Progress snapshot ────────────────────────────────
    let progress_snapshot = session
        .progress_snapshot
        .as_ref()
        .and_then(|items| serde_json::to_value(items).ok());

    // ── Assemble response ────────────────────────────────
    let mut response = serde_json::json!({
        "status": "recovered",
        "session_id": session.id,
    });

    if !pending_requests.is_empty() {
        response["pending_requests"] = serde_json::json!(pending_requests);
    }
    if let Some(cp) = last_checkpoint {
        response["last_checkpoint"] = cp;
    }
    if let Some(snap) = progress_snapshot {
        response["progress_snapshot"] = snap;
    }

    Ok(response)
}

/// Wrap a JSON value into a successful `CallToolResult`.
fn json_result(value: serde_json::Value) -> Result<CallToolResult, rmcp::ErrorData> {
    Ok(CallToolResult::success(vec![rmcp::model::Content::json(
        value,
    )
    .map_err(|err| {
        rmcp::ErrorData::internal_error(
            format!("failed to serialize recovery response: {err}"),
            None,
        )
    })?]))
}
