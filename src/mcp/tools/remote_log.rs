//! `remote_log` MCP tool handler (T055, T056).
//!
//! Sends a non-blocking status log message to the Slack channel with
//! severity-based formatting. Returns immediately without waiting for
//! operator action.

use std::sync::Arc;

use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::CallToolResult;
use slack_morphism::prelude::{SlackChannelId, SlackTs};
use tracing::{info, info_span, warn, Instrument};

use crate::mcp::handler::AgentRemServer;
use crate::persistence::session_repo::SessionRepo;
use crate::slack::blocks;
use crate::slack::client::SlackMessage;

/// Input parameters per mcp-tools.json contract.
#[derive(Debug, serde::Deserialize)]
struct RemoteLogInput {
    /// Log message to post.
    message: String,
    /// Severity level controlling visual presentation (default: `info`).
    #[serde(default = "default_level")]
    level: String,
    /// Optional Slack thread timestamp to post as a reply.
    thread_ts: Option<String>,
}

fn default_level() -> String {
    "info".to_owned()
}

/// Valid severity levels.
const VALID_LEVELS: &[&str] = &["info", "success", "warning", "error"];

/// Handle the `remote_log` tool call.
///
/// # Errors
///
/// Returns `rmcp::ErrorData` on validation or infrastructure failures.
pub async fn handle(
    context: ToolCallContext<'_, AgentRemServer>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let state = Arc::clone(context.service.state());
    let channel_id = context.service.effective_channel_id().to_owned();
    let args: serde_json::Map<String, serde_json::Value> = context.arguments.unwrap_or_default();

    let input: RemoteLogInput =
        serde_json::from_value(serde_json::Value::Object(args)).map_err(|err| {
            rmcp::ErrorData::invalid_params(format!("invalid remote_log parameters: {err}"), None)
        })?;

    let span = info_span!(
        "remote_log",
        level = %input.level,
        has_thread = input.thread_ts.is_some(),
    );

    async move {
        // ── Validate level ───────────────────────────────────
        if !VALID_LEVELS.contains(&input.level.as_str()) {
            return Err(rmcp::ErrorData::invalid_params(
                format!(
                    "invalid level '{}'; expected one of: {}",
                    input.level,
                    VALID_LEVELS.join(", ")
                ),
                None,
            ));
        }

        // ── Resolve active session (for last_tool update) ────
        let session_repo = SessionRepo::new(Arc::clone(&state.db));
        let sessions = session_repo.list_active().await.map_err(|err| {
            rmcp::ErrorData::internal_error(format!("failed to query active sessions: {err}"), None)
        })?;
        let session = sessions
            .into_iter()
            .next()
            .ok_or_else(|| rmcp::ErrorData::internal_error("no active session found", None))?;

        // ── Post to Slack ────────────────────────────────────
        let (posted, ts) = if let Some(ref slack) = state.slack {
            let channel = SlackChannelId(channel_id.clone());
            let thread_ts = input.thread_ts.as_ref().map(|ts| SlackTs(ts.clone()));

            let severity_block = blocks::severity_section(&input.level, &input.message);
            let msg = SlackMessage {
                channel,
                text: Some(input.message.clone()),
                blocks: Some(vec![severity_block]),
                thread_ts,
            };

            match slack.post_message_direct(msg).await {
                Ok(slack_ts) => {
                    info!(ts = %slack_ts.0, "remote_log posted to slack");
                    (true, slack_ts.0)
                }
                Err(err) => {
                    warn!(%err, "failed to post remote_log to slack");
                    (false, String::new())
                }
            }
        } else {
            warn!("slack not configured; remote_log message not posted");
            (false, String::new())
        };

        // ── Update session last_tool ─────────────────────────
        let _ = session_repo
            .update_last_activity(&session.id, Some("remote_log".to_owned()))
            .await;

        info!(
            session_id = %session.id,
            posted,
            level = %input.level,
            "remote_log completed"
        );

        // ── Build response ───────────────────────────────────
        let response = serde_json::json!({
            "posted": posted,
            "ts": ts,
        });

        Ok(CallToolResult::success(vec![rmcp::model::Content::json(
            response,
        )
        .map_err(|err| {
            rmcp::ErrorData::internal_error(
                format!("failed to serialize remote_log response: {err}"),
                None,
            )
        })?]))
    }
    .instrument(span)
    .await
}
