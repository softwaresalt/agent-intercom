//! `set_operational_mode` MCP tool handler (T084).
//!
//! Switches the server between remote, local, and hybrid operational
//! modes at runtime. Updates the session mode in the database so the
//! change persists across restarts. Returns the previous and current
//! modes.

use std::sync::Arc;

use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::CallToolResult;
use tracing::{info, info_span, Instrument};

use crate::mcp::handler::IntercomServer;
use crate::models::session::SessionMode;
use crate::persistence::session_repo::SessionRepo;

/// Input parameters per mcp-tools.json contract.
#[derive(Debug, serde::Deserialize)]
struct SetModeInput {
    /// Target mode: `remote`, `local`, or `hybrid`.
    mode: SessionMode,
}

/// Handle the `set_operational_mode` tool call.
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

    let input: SetModeInput =
        serde_json::from_value(serde_json::Value::Object(args)).map_err(|err| {
            rmcp::ErrorData::invalid_params(
                format!("invalid set_operational_mode parameters: {err}"),
                None,
            )
        })?;

    let span = info_span!(
        "set_operational_mode",
        target_mode = ?input.mode,
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

        let previous_mode = session.mode;

        // ── Update mode in DB ────────────────────────────────
        session_repo
            .update_mode(&session.id, input.mode)
            .await
            .map_err(|err| {
                rmcp::ErrorData::internal_error(
                    format!("failed to update session mode: {err}"),
                    None,
                )
            })?;

        // ── Update last activity ─────────────────────────────
        let _ = session_repo
            .update_last_activity(&session.id, Some("set_operational_mode".to_owned()))
            .await;

        // ── Notify Slack if connected ────────────────────────
        if let (Some(ref slack), Some(ref ch)) = (&state.slack, &channel_id) {
            // Only post to Slack if the new mode still includes Slack.
            if matches!(input.mode, SessionMode::Remote | SessionMode::Hybrid) {
                let channel = slack_morphism::prelude::SlackChannelId(ch.clone());
                let msg = crate::slack::client::SlackMessage {
                    channel,
                    text: Some(format!(
                        "\u{1f504} Mode changed: {prev} \u{2192} {curr}",
                        prev = mode_label(previous_mode),
                        curr = mode_label(input.mode),
                    )),
                    blocks: Some(vec![crate::slack::blocks::severity_section(
                        "info",
                        &format!(
                            "Operational mode changed: *{}* \u{2192} *{}*",
                            mode_label(previous_mode),
                            mode_label(input.mode),
                        ),
                    )]),
                    thread_ts: None,
                };
                if let Err(err) = slack.enqueue(msg).await {
                    tracing::warn!(%err, "failed to notify mode change to slack");
                }
            }
        }

        info!(
            session_id = %session.id,
            ?previous_mode,
            current_mode = ?input.mode,
            "operational mode changed"
        );

        // ── Build response ───────────────────────────────────
        let response = serde_json::json!({
            "previous_mode": mode_str(previous_mode),
            "current_mode": mode_str(input.mode),
        });

        Ok(CallToolResult::success(vec![rmcp::model::Content::json(
            response,
        )
        .map_err(|err| {
            rmcp::ErrorData::internal_error(
                format!("failed to serialize mode response: {err}"),
                None,
            )
        })?]))
    }
    .instrument(span)
    .await
}

/// Human-readable label for a session mode.
fn mode_label(mode: SessionMode) -> &'static str {
    match mode {
        SessionMode::Remote => "Remote",
        SessionMode::Local => "Local",
        SessionMode::Hybrid => "Hybrid",
    }
}

/// JSON string representation for a session mode.
fn mode_str(mode: SessionMode) -> &'static str {
    match mode {
        SessionMode::Remote => "remote",
        SessionMode::Local => "local",
        SessionMode::Hybrid => "hybrid",
    }
}
