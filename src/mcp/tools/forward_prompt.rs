//! `forward_prompt` MCP tool handler (T057, T060).
//!
//! Forwards an agent-generated continuation prompt to the remote operator
//! via Slack with Continue/Refine/Stop buttons. Blocks the agent until
//! the operator responds or the configured timeout elapses.

use std::sync::Arc;
use std::time::Duration;

use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::CallToolResult;
use slack_morphism::prelude::{SlackBlock, SlackChannelId};
use tokio::sync::oneshot;
use tracing::{info, info_span, warn, Instrument};

use crate::mcp::handler::{IntercomServer, PromptResponse};
use crate::models::prompt::{ContinuationPrompt, PromptDecision, PromptType};
use crate::persistence::prompt_repo::PromptRepo;
use crate::persistence::session_repo::SessionRepo;
use crate::slack::blocks;
use crate::slack::client::SlackMessage;

use super::util::truncate_text;

/// Input parameters for the `forward_prompt` tool per mcp-tools.json contract.
#[derive(Debug, serde::Deserialize)]
struct ForwardPromptInput {
    /// Raw text of the continuation prompt.
    prompt_text: String,
    /// Category of the prompt.
    #[serde(default = "default_prompt_type")]
    prompt_type: PromptType,
    /// Seconds since last user interaction.
    elapsed_seconds: Option<i64>,
    /// Count of actions performed in this iteration.
    actions_taken: Option<i64>,
}

fn default_prompt_type() -> PromptType {
    PromptType::Continuation
}

/// Handle the `forward_prompt` tool call.
///
/// # Errors
///
/// Returns `rmcp::ErrorData` on validation or infrastructure failures.
#[allow(clippy::too_many_lines)] // Prompt flow is inherently sequential with many steps.
pub async fn handle(
    context: ToolCallContext<'_, IntercomServer>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let state = Arc::clone(context.service.state());
    let channel_id = context.service.effective_channel_id().map(str::to_owned);
    let args: serde_json::Map<String, serde_json::Value> = context.arguments.unwrap_or_default();

    let input: ForwardPromptInput = serde_json::from_value(serde_json::Value::Object(args))
        .map_err(|err| {
            rmcp::ErrorData::invalid_params(
                format!("invalid forward_prompt parameters: {err}"),
                None,
            )
        })?;

    let span = info_span!(
        "forward_prompt",
        prompt_type = ?input.prompt_type,
    );

    async move {
        // ── Resolve session ──────────────────────────────────
        let session_repo = SessionRepo::new(Arc::clone(&state.db));
        let sessions = session_repo.list_active().await.map_err(|err| {
            rmcp::ErrorData::internal_error(format!("failed to query active sessions: {err}"), None)
        })?;
        let session = sessions
            .into_iter()
            .next()
            .ok_or_else(|| rmcp::ErrorData::internal_error("no active session found", None))?;

        // ── Create ContinuationPrompt record ─────────────────
        let prompt = ContinuationPrompt::new(
            session.id.clone(),
            input.prompt_text.clone(),
            input.prompt_type,
            input.elapsed_seconds,
            input.actions_taken,
        );
        let prompt_id = prompt.id.clone();

        let prompt_repo = PromptRepo::new(Arc::clone(&state.db));
        let created = prompt_repo.create(&prompt).await.map_err(|err| {
            rmcp::ErrorData::internal_error(
                format!("failed to persist continuation prompt: {err}"),
                None,
            )
        })?;

        // ── Post to Slack ────────────────────────────────────
        if let (Some(ref slack), Some(ref ch)) = (&state.slack, &channel_id) {
            let channel = SlackChannelId(ch.clone());
            let message_blocks = build_prompt_blocks(
                &input.prompt_text,
                input.prompt_type,
                input.elapsed_seconds,
                input.actions_taken,
                &prompt_id,
            );

            let post_span = info_span!("slack_post_prompt", prompt_id = %prompt_id);
            async {
                let msg = SlackMessage {
                    channel,
                    text: Some(format!(
                        "\u{1f4ac} {} Prompt: {}",
                        prompt_type_label(input.prompt_type),
                        truncate_text(&input.prompt_text, 100),
                    )),
                    blocks: Some(message_blocks),
                    thread_ts: None,
                };
                if let Err(err) = slack.enqueue(msg).await {
                    warn!(%err, "failed to enqueue prompt message");
                }
            }
            .instrument(post_span)
            .await;
        } else {
            warn!("slack not configured; prompt will block without notification");
        }

        // Suppress unused variable warning.
        let _ = &created;

        // ── Register oneshot and wait ────────────────────────
        let (tx, rx) = oneshot::channel::<PromptResponse>();
        {
            let mut pending = state.pending_prompts.lock().await;
            pending.insert(prompt_id.clone(), tx);
        }

        let timeout_seconds = state.config.timeouts.prompt_seconds;
        let timeout_duration = Duration::from_secs(timeout_seconds);

        let response = tokio::time::timeout(timeout_duration, rx).await;

        let (decision, instruction) = match response {
            Ok(Ok(resp)) => (resp.decision, resp.instruction),
            Ok(Err(_)) => {
                // Sender dropped without sending (e.g., server shutdown).
                // Default to "continue" per FR-008.
                ("continue".to_owned(), None)
            }
            Err(_elapsed) => {
                // Timeout expired — auto-respond with "continue" per FR-008.
                info!(
                    prompt_id = %prompt_id,
                    timeout_seconds,
                    "continuation prompt timed out; auto-continuing"
                );
                let _ = prompt_repo
                    .update_decision(&prompt_id, PromptDecision::Continue, None)
                    .await;

                if let (Some(ref slack), Some(ref ch)) = (&state.slack, &channel_id) {
                    let channel = SlackChannelId(ch.clone());
                    let msg = SlackMessage {
                        channel,
                        text: Some(format!(
                            "\u{23f1}\u{fe0f} Prompt '{}' timed out \u{2014} auto-continuing",
                            truncate_text(&input.prompt_text, 60),
                        )),
                        blocks: Some(vec![blocks::severity_section(
                            "warning",
                            &format!("Prompt timed out after {timeout_seconds}s — auto-continuing"),
                        )]),
                        thread_ts: None,
                    };
                    let _ = slack.enqueue(msg).await;
                }

                ("continue".to_owned(), None)
            }
        };

        // Clean up pending map.
        {
            let mut pending = state.pending_prompts.lock().await;
            pending.remove(&prompt_id);
        }

        // Update session last_tool.
        let _ = session_repo
            .update_last_activity(&session.id, Some("forward_prompt".to_owned()))
            .await;

        info!(
            prompt_id = %prompt_id,
            decision = %decision,
            "forward_prompt resolved"
        );

        // ── Build response ───────────────────────────────────
        let mut response_json = serde_json::json!({
            "decision": decision,
        });
        if let Some(ref inst) = instruction {
            response_json["instruction"] = serde_json::Value::String(inst.clone());
        }

        Ok(CallToolResult::success(vec![rmcp::model::Content::json(
            response_json,
        )
        .map_err(|err| {
            rmcp::ErrorData::internal_error(
                format!("failed to serialize forward_prompt response: {err}"),
                None,
            )
        })?]))
    }
    .instrument(span)
    .await
}

/// Build Slack Block Kit blocks for the prompt message.
fn build_prompt_blocks(
    prompt_text: &str,
    prompt_type: PromptType,
    elapsed_seconds: Option<i64>,
    actions_taken: Option<i64>,
    prompt_id: &str,
) -> Vec<SlackBlock> {
    let mut result = Vec::new();

    // Header with prompt type icon.
    let icon = prompt_type_icon(prompt_type);
    let label = prompt_type_label(prompt_type);
    result.push(blocks::text_section(&format!("{icon} *{label} Prompt*")));

    // Prompt text.
    result.push(blocks::text_section(prompt_text));

    // Context line with elapsed time and actions.
    let mut context_parts = Vec::new();
    if let Some(secs) = elapsed_seconds {
        context_parts.push(format!("\u{23f1}\u{fe0f} {secs}s elapsed"));
    }
    if let Some(count) = actions_taken {
        context_parts.push(format!("\u{1f4cb} {count} actions taken"));
    }
    if !context_parts.is_empty() {
        result.push(blocks::text_section(&context_parts.join(" | ")));
    }

    // Action buttons.
    result.push(blocks::prompt_buttons(prompt_id));

    result
}

/// Get the display icon for a prompt type.
fn prompt_type_icon(prompt_type: PromptType) -> &'static str {
    match prompt_type {
        PromptType::Continuation => "\u{1f504}",
        PromptType::Clarification => "\u{2753}",
        PromptType::ErrorRecovery => "\u{26a0}\u{fe0f}",
        PromptType::ResourceWarning => "\u{1f4ca}",
    }
}

/// Get the display label for a prompt type.
fn prompt_type_label(prompt_type: PromptType) -> &'static str {
    match prompt_type {
        PromptType::Continuation => "Continuation",
        PromptType::Clarification => "Clarification",
        PromptType::ErrorRecovery => "Error Recovery",
        PromptType::ResourceWarning => "Resource Warning",
    }
}
