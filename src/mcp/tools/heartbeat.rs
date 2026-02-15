//! `heartbeat` MCP tool handler (T049).
//!
//! Lightweight liveness signal that resets the stall detection timer
//! and optionally stores a structured progress snapshot on the session.

use std::sync::Arc;

use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::CallToolResult;
use tracing::{info, info_span, Instrument};

use crate::mcp::handler::AgentRemServer;
use crate::models::progress::{validate_snapshot, ProgressItem};
use crate::persistence::session_repo::SessionRepo;

/// Input parameters per mcp-tools.json contract.
#[derive(Debug, serde::Deserialize)]
struct HeartbeatInput {
    /// Optional status update logged to the operator.
    status_message: Option<String>,
    /// Optional structured progress snapshot (replaces previous when present).
    progress_snapshot: Option<Vec<ProgressItem>>,
}

/// Handle the `heartbeat` tool call.
///
/// # Errors
///
/// Returns `rmcp::ErrorData` on validation or persistence failures.
pub async fn handle(
    context: ToolCallContext<'_, AgentRemServer>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let state = Arc::clone(context.service.state());
    let channel_id = context.service.effective_channel_id().to_owned();
    let args: serde_json::Map<String, serde_json::Value> = context.arguments.unwrap_or_default();

    let input: HeartbeatInput =
        serde_json::from_value(serde_json::Value::Object(args)).map_err(|err| {
            rmcp::ErrorData::invalid_params(format!("invalid heartbeat parameters: {err}"), None)
        })?;

    let span = info_span!(
        "heartbeat",
        has_snapshot = input.progress_snapshot.is_some(),
        has_message = input.status_message.is_some(),
    );

    async move {
        // ── Resolve active session ───────────────────────────
        let session_repo = SessionRepo::new(Arc::clone(&state.db));
        let sessions = session_repo.list_active().await.map_err(|err| {
            rmcp::ErrorData::internal_error(format!("failed to query active sessions: {err}"), None)
        })?;
        let session = sessions
            .into_iter()
            .next()
            .ok_or_else(|| rmcp::ErrorData::internal_error("no active session found", None))?;

        let stall_enabled = state.config.stall.enabled;

        // ── Validate snapshot if provided ────────────────────
        if let Some(ref snapshot) = input.progress_snapshot {
            validate_snapshot(snapshot).map_err(|err| {
                rmcp::ErrorData::invalid_params(format!("invalid progress snapshot: {err}"), None)
            })?;
        }

        // ── Update session progress if snapshot provided ─────
        if input.progress_snapshot.is_some() {
            session_repo
                .update_progress_snapshot(&session.id, input.progress_snapshot.clone())
                .await
                .map_err(|err| {
                    rmcp::ErrorData::internal_error(
                        format!("failed to update progress snapshot: {err}"),
                        None,
                    )
                })?;
        }

        // ── Update last activity ─────────────────────────────
        session_repo
            .update_last_activity(&session.id, Some("heartbeat".into()))
            .await
            .map_err(|err| {
                rmcp::ErrorData::internal_error(
                    format!("failed to update session activity: {err}"),
                    None,
                )
            })?;

        // ── Reset stall timer via shared state ───────────────
        if let Some(ref detectors) = state.stall_detectors {
            let guards = detectors.lock().await;
            if let Some(detector_handle) = guards.get(&session.id) {
                detector_handle.reset();
                info!(session_id = %session.id, "stall timer reset by heartbeat");
            }
        }

        // ── Optional: log status_message to Slack ────────────
        if let Some(ref msg) = input.status_message {
            if let Some(ref slack) = state.slack {
                let channel = slack_morphism::prelude::SlackChannelId(channel_id.clone());
                let slack_msg = crate::slack::client::SlackMessage {
                    channel,
                    text: Some(format!("\u{1f493} {msg}")),
                    blocks: Some(vec![crate::slack::blocks::severity_section("info", msg)]),
                    thread_ts: None,
                };
                if let Err(err) = slack.enqueue(slack_msg).await {
                    tracing::warn!(%err, "failed to enqueue heartbeat status to slack");
                }
            }
        }

        info!(
            session_id = %session.id,
            stall_enabled,
            "heartbeat acknowledged"
        );

        // ── Build response ───────────────────────────────────
        let response = serde_json::json!({
            "acknowledged": true,
            "session_id": session.id,
            "stall_detection_enabled": stall_enabled,
        });

        Ok(CallToolResult::success(vec![rmcp::model::Content::json(
            response,
        )
        .map_err(|err| {
            rmcp::ErrorData::internal_error(
                format!("failed to serialize heartbeat response: {err}"),
                None,
            )
        })?]))
    }
    .instrument(span)
    .await
}
