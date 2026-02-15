//! `wait_for_instruction` MCP tool handler (T086).
//!
//! Places the agent in standby, posting a waiting status to Slack with
//! Resume/Stop buttons. Blocks until the operator responds via Slack
//! (or IPC) or the configured timeout elapses. Returns the operator's
//! instruction or a timeout status.

use std::sync::Arc;
use std::time::Duration;

use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::CallToolResult;
use slack_morphism::prelude::SlackChannelId;
use tokio::sync::oneshot;
use tracing::{info, info_span, warn, Instrument};

use crate::mcp::handler::{AgentRemServer, WaitResponse};
use crate::persistence::session_repo::SessionRepo;
use crate::slack::blocks;
use crate::slack::client::SlackMessage;

/// Default status message when none is provided.
const DEFAULT_WAIT_MESSAGE: &str = "Agent is idle and awaiting instructions.";

/// Input parameters per mcp-tools.json contract.
#[derive(Debug, serde::Deserialize)]
struct WaitInput {
    /// Status message displayed in Slack while waiting.
    #[serde(default = "default_message")]
    message: String,
    /// Maximum wait time in seconds. 0 = indefinite.
    #[serde(default)]
    timeout_seconds: u64,
}

fn default_message() -> String {
    DEFAULT_WAIT_MESSAGE.to_owned()
}

/// Handle the `wait_for_instruction` tool call.
///
/// # Errors
///
/// Returns `rmcp::ErrorData` on validation or infrastructure failures.
#[allow(clippy::too_many_lines)] // Wait flow is inherently sequential with many steps.
pub async fn handle(
    context: ToolCallContext<'_, AgentRemServer>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let state = Arc::clone(context.service.state());
    let channel_id = context.service.effective_channel_id().to_owned();
    let args: serde_json::Map<String, serde_json::Value> = context.arguments.unwrap_or_default();

    let input: WaitInput =
        serde_json::from_value(serde_json::Value::Object(args)).map_err(|err| {
            rmcp::ErrorData::invalid_params(
                format!("invalid wait_for_instruction parameters: {err}"),
                None,
            )
        })?;

    let span = info_span!(
        "wait_for_instruction",
        timeout_seconds = input.timeout_seconds,
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

        // ── Post waiting status to Slack ─────────────────────
        if let Some(ref slack) = state.slack {
            let channel = SlackChannelId(channel_id.clone());
            let mut message_blocks = vec![blocks::text_section(&format!(
                "\u{23f8}\u{fe0f} *Agent Waiting*\n{}",
                &input.message,
            ))];
            if input.timeout_seconds > 0 {
                message_blocks.push(blocks::text_section(&format!(
                    "\u{23f1}\u{fe0f} Timeout: {}s",
                    input.timeout_seconds,
                )));
            }
            message_blocks.push(blocks::wait_buttons(&session.id));

            let msg = SlackMessage {
                channel,
                text: Some(format!(
                    "\u{23f8}\u{fe0f} Agent waiting: {}",
                    truncate_text(&input.message, 100),
                )),
                blocks: Some(message_blocks),
                thread_ts: None,
            };
            if let Err(err) = slack.enqueue(msg).await {
                warn!(%err, "failed to enqueue wait status to slack");
            }
        } else {
            warn!("slack not configured; wait will block without notification");
        }

        // ── Register oneshot and wait ────────────────────────
        let (tx, rx) = oneshot::channel::<WaitResponse>();
        {
            let mut pending = state.pending_waits.lock().await;
            pending.insert(session.id.clone(), tx);
        }

        // Determine effective timeout.
        let effective_timeout = if input.timeout_seconds == 0 {
            // Use config wait_seconds; 0 means truly indefinite.
            state.config.timeouts.wait_seconds
        } else {
            input.timeout_seconds
        };

        let response = if effective_timeout == 0 {
            // Indefinite wait — no timeout.
            match rx.await {
                Ok(resp) => resp,
                Err(_) => WaitResponse {
                    status: "timeout".to_owned(),
                    instruction: None,
                },
            }
        } else {
            let timeout_duration = Duration::from_secs(effective_timeout);
            match tokio::time::timeout(timeout_duration, rx).await {
                Ok(Ok(resp)) => resp,
                Ok(Err(_)) => {
                    // Sender dropped without sending.
                    WaitResponse {
                        status: "timeout".to_owned(),
                        instruction: None,
                    }
                }
                Err(_elapsed) => {
                    info!(
                        session_id = %session.id,
                        timeout_seconds = effective_timeout,
                        "wait_for_instruction timed out"
                    );

                    // Notify Slack of timeout.
                    if let Some(ref slack) = state.slack {
                        let channel = SlackChannelId(channel_id.clone());
                        let msg = SlackMessage {
                            channel,
                            text: Some(format!(
                                "\u{23f1}\u{fe0f} Wait timed out after {effective_timeout}s"
                            )),
                            blocks: Some(vec![blocks::severity_section(
                                "warning",
                                &format!(
                                    "Wait timed out after {effective_timeout}s — agent resuming"
                                ),
                            )]),
                            thread_ts: None,
                        };
                        let _ = slack.enqueue(msg).await;
                    }

                    WaitResponse {
                        status: "timeout".to_owned(),
                        instruction: None,
                    }
                }
            }
        };

        // Clean up pending map.
        {
            let mut pending = state.pending_waits.lock().await;
            pending.remove(&session.id);
        }

        // Update session last_tool.
        let _ = session_repo
            .update_last_activity(&session.id, Some("wait_for_instruction".to_owned()))
            .await;

        info!(
            session_id = %session.id,
            status = %response.status,
            "wait_for_instruction resolved"
        );

        // ── Build response ───────────────────────────────────
        let mut response_json = serde_json::json!({
            "status": response.status,
        });
        if let Some(ref inst) = response.instruction {
            response_json["instruction"] = serde_json::Value::String(inst.clone());
        }

        Ok(CallToolResult::success(vec![rmcp::model::Content::text(
            serde_json::to_string(&response_json).unwrap_or_default(),
        )]))
    }
    .instrument(span)
    .await
}

/// Truncate text to a maximum length, appending "..." if truncated.
fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_owned()
    } else {
        format!("{}...", &text[..max_len.saturating_sub(3)])
    }
}
