//! `forward_prompt` MCP tool handler (T057, T060).
//!
//! Forwards an agent-generated continuation prompt to the remote operator
//! via Slack with Continue/Refine/Stop buttons. Blocks the agent until
//! the operator responds or the configured timeout elapses.

use std::sync::Arc;
use std::time::Duration;

use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::CallToolResult;
use slack_morphism::prelude::SlackChannelId;
use tokio::sync::oneshot;
use tracing::{info, info_span, warn, Instrument};

use crate::mcp::handler::{IntercomServer, PromptResponse};
use crate::models::prompt::{ContinuationPrompt, PromptDecision, PromptType};
use crate::persistence::prompt_repo::PromptRepo;
use crate::persistence::session_repo::SessionRepo;
use crate::slack::blocks;
use crate::slack::client::SlackMessage;

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
        // ── Early Slack channel check (T067 / S040) ────────
        // Return a descriptive error instead of blocking indefinitely when
        // no Slack channel is configured for this session.
        if state.slack.is_none() || channel_id.is_none() {
            let (error_code, error_message) = if state.slack.is_none() {
                (
                    "slack_unavailable",
                    "Slack service is not configured; transmit requires Slack",
                )
            } else {
                (
                    "no_channel",
                    "no Slack channel configured for this session; \
                     set channel_id in the /mcp URL query string to enable prompt forwarding",
                )
            };
            let body = serde_json::json!({
                "status": "error",
                "error_code": error_code,
                "error_message": error_message,
            });
            return Ok(CallToolResult::success(vec![rmcp::model::Content::json(
                body,
            )
            .unwrap_or_else(|_| {
                rmcp::model::Content::text(format!("{error_code}: {error_message}"))
            })]));
        }

        // ── Resolve session ──────────────────────────────────
        // In ACP mode the agent subprocess supplies `?session_id=<id>` so we
        // can pin the tool call to the exact session. Fall back to the first
        // active session for backwards-compatible MCP mode.
        let session_repo = SessionRepo::new(Arc::clone(&state.db));
        let session = if let Some(sid) = context.service.session_id_override() {
            session_repo
                .get_by_id(sid)
                .await
                .map_err(|err| {
                    rmcp::ErrorData::internal_error(format!("failed to query session: {err}"), None)
                })?
                .ok_or_else(|| rmcp::ErrorData::internal_error("session not found", None))?
        } else {
            let sessions = session_repo.list_active().await.map_err(|err| {
                rmcp::ErrorData::internal_error(
                    format!("failed to query active sessions: {err}"),
                    None,
                )
            })?;
            sessions
                .into_iter()
                .next()
                .ok_or_else(|| rmcp::ErrorData::internal_error("no active session found", None))?
        };

        // S037: capture thread_ts so all outgoing messages go to the session thread.
        let session_thread_ts = session
            .thread_ts
            .as_deref()
            .map(|ts| slack_morphism::prelude::SlackTs(ts.to_owned()));

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
        // US17: when the session lives in a thread, post plain text (no
        // block-kit buttons) and use the @-mention thread-reply mechanism
        // for operator decisions.  Main channel messages keep block-kit.
        let is_threaded = session_thread_ts.is_some();

        if let (Some(ref slack), Some(ref ch)) = (&state.slack, &channel_id) {
            let channel = SlackChannelId(ch.clone());

            if is_threaded {
                // US17: text-only thread prompt — no blocks.
                let text_body = blocks::build_text_only_prompt(
                    &input.prompt_text,
                    input.prompt_type,
                    input.elapsed_seconds,
                    input.actions_taken,
                );
                let msg = SlackMessage {
                    channel,
                    text: Some(text_body),
                    blocks: None,
                    thread_ts: session_thread_ts.clone(),
                };
                if let Err(err) = slack.enqueue(msg).await {
                    warn!(%err, "failed to enqueue text-only prompt message");
                }
            } else {
                // Main channel: block-kit with buttons (unchanged).
                let message_blocks = blocks::build_prompt_blocks(
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
                            blocks::prompt_type_label(input.prompt_type),
                            blocks::truncate_text(&input.prompt_text, 100),
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
            }
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

        // US17: register thread-reply fallback so an @-mention reply in
        // the thread resolves the prompt via `driver.resolve_prompt()`.
        if is_threaded {
            if let Some(ref ch) = channel_id {
                let thread_ts = session_thread_ts
                    .as_ref()
                    .map(|ts| ts.0.clone())
                    .unwrap_or_default();
                let authorized_user = session.owner_user_id.clone();
                let fallback_ch = ch.clone();
                let fallback_ts = thread_ts.clone();

                let (reply_tx, reply_rx) = oneshot::channel::<String>();
                let registered =
                    crate::slack::handlers::thread_reply::register_thread_reply_fallback(
                        ch,
                        thread_ts,
                        session.id.clone(),
                        authorized_user,
                        reply_tx,
                        Arc::clone(&state.pending_thread_replies),
                    )
                    .await;

                if registered {
                    // Spawn a task that waits for the @-mention text and
                    // resolves the prompt through the driver.
                    let state_fb = Arc::clone(&state);
                    let pid = prompt_id.clone();
                    tokio::spawn(async move {
                        let timeout = Duration::from_secs(state_fb.config.timeouts.prompt_seconds);
                        match tokio::time::timeout(timeout, reply_rx).await {
                            Ok(Ok(reply_text)) => {
                                let decision =
                                    crate::slack::handlers::thread_reply::parse_thread_decision(
                                        &reply_text,
                                    );
                                let inst = if decision.instruction.is_empty() {
                                    None
                                } else {
                                    Some(decision.instruction)
                                };
                                if let Err(err) = state_fb
                                    .driver
                                    .resolve_prompt(&pid, &decision.keyword, inst)
                                    .await
                                {
                                    warn!(
                                        prompt_id = pid,
                                        %err,
                                        "US17: failed to resolve prompt from thread reply"
                                    );
                                }
                            }
                            Ok(Err(_)) => {
                                // Sender dropped (e.g., cleanup_session_fallbacks) — no-op.
                            }
                            Err(_elapsed) => {
                                // Timeout — remove stale entry so future registrations
                                // for this thread are not blocked by LC-04.
                                state_fb.pending_thread_replies.lock().await.remove(
                                    &crate::slack::handlers::thread_reply::fallback_map_key(
                                        &fallback_ch,
                                        &fallback_ts,
                                    ),
                                );
                            }
                        }
                    });
                }
            }
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
                            blocks::truncate_text(&input.prompt_text, 60),
                        )),
                        blocks: Some(vec![blocks::severity_section(
                            "warning",
                            &format!("Prompt timed out after {timeout_seconds}s — auto-continuing"),
                        )]),
                        thread_ts: session_thread_ts.clone(),
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
