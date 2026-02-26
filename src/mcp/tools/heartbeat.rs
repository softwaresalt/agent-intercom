//! `heartbeat` MCP tool handler (T049).
//!
//! Lightweight liveness signal that resets the stall detection timer
//! and optionally stores a structured progress snapshot on the session.

use std::sync::Arc;

use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::CallToolResult;
use tracing::{info, info_span, Instrument};

use crate::mcp::handler::IntercomServer;
use crate::models::progress::{validate_snapshot, ProgressItem};
use crate::persistence::session_repo::SessionRepo;
use crate::persistence::steering_repo::SteeringRepo;
use crate::slack::client::SlackMessage;

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
    context: ToolCallContext<'_, IntercomServer>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let state = Arc::clone(context.service.state());
    let channel_id = context.service.effective_channel_id().map(str::to_owned);
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

        // Avoid arbitrarily selecting an active session when multiple exist.
        let mut iter = sessions.into_iter();
        let first = iter.next();
        let second = iter.next();
        let session = match (first, second) {
            (None, _) => {
                return Err(rmcp::ErrorData::internal_error(
                    "no active session found",
                    None,
                ));
            }
            (Some(sess), None) => sess,
            (Some(_), Some(_)) => {
                return Err(rmcp::ErrorData::internal_error(
                    "multiple active sessions found; heartbeat requires an unambiguous session",
                    None,
                ));
            }
        };

        let stall_enabled = state.config.stall.enabled;

        // ── Update session snapshot and activity ──────────────
        update_session_progress(&session_repo, &session.id, &input).await?;

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
            if let Some(ref ch) = channel_id {
                send_heartbeat_to_slack(&state, ch, msg).await;
            }
        }

        // ── Fetch and deliver pending steering messages ──────
        let steering_repo = SteeringRepo::new(Arc::clone(&state.db));
        let steering_texts = fetch_and_consume_steering(&steering_repo, &session.id).await?;

        info!(
            session_id = %session.id,
            stall_enabled,
            steering_count = steering_texts.len(),
            "heartbeat acknowledged"
        );

        // ── Build response ───────────────────────────────────
        let response = serde_json::json!({
            "acknowledged": true,
            "session_id": session.id,
            "stall_detection_enabled": stall_enabled,
            "pending_steering": steering_texts,
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

/// Validate the progress snapshot (if present) and update session persistence.
async fn update_session_progress(
    session_repo: &SessionRepo,
    session_id: &str,
    input: &HeartbeatInput,
) -> Result<(), rmcp::ErrorData> {
    if let Some(ref snapshot) = input.progress_snapshot {
        validate_snapshot(snapshot).map_err(|err| {
            rmcp::ErrorData::invalid_params(format!("invalid progress snapshot: {err}"), None)
        })?;
        session_repo
            .update_progress_snapshot(session_id, Some(snapshot.clone()))
            .await
            .map_err(|err| {
                rmcp::ErrorData::internal_error(
                    format!("failed to update progress snapshot: {err}"),
                    None,
                )
            })?;
    }
    session_repo
        .update_last_activity(session_id, Some("heartbeat".into()))
        .await
        .map_err(|err| {
            rmcp::ErrorData::internal_error(
                format!("failed to update session activity: {err}"),
                None,
            )
        })
}

/// Fetch all unconsumed steering messages for the session and mark them consumed.
///
/// Returns a `Vec<String>` of message texts in insertion order.
async fn fetch_and_consume_steering(
    repo: &SteeringRepo,
    session_id: &str,
) -> Result<Vec<String>, rmcp::ErrorData> {
    let pending = repo.fetch_unconsumed(session_id).await.map_err(|err| {
        rmcp::ErrorData::internal_error(format!("failed to fetch steering messages: {err}"), None)
    })?;
    let texts: Vec<String> = pending.iter().map(|m| m.message.clone()).collect();
    for msg in &pending {
        repo.mark_consumed(&msg.id).await.map_err(|err| {
            rmcp::ErrorData::internal_error(
                format!("failed to mark steering message consumed: {err}"),
                None,
            )
        })?;
    }
    Ok(texts)
}

/// Forward a heartbeat status message to the Slack channel, if configured.
async fn send_heartbeat_to_slack(
    state: &crate::mcp::handler::AppState,
    channel_id: &str,
    msg: &str,
) {
    if let Some(ref slack) = state.slack {
        let channel = slack_morphism::prelude::SlackChannelId(channel_id.to_owned());
        let slack_msg = SlackMessage {
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
