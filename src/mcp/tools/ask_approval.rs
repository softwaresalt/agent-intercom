//! `ask_approval` MCP tool handler (T038, T040, T042).
//!
//! Submits a code proposal for remote operator approval via Slack.
//! Blocks the agent until the operator responds (Accept/Reject) or
//! the configured timeout elapses.

use std::sync::Arc;
use std::time::Duration;

use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::CallToolResult;
use slack_morphism::prelude::{SlackBlock, SlackChannelId};
use tokio::sync::oneshot;
use tracing::{info, info_span, warn, Instrument};

use crate::mcp::handler::{AgentRcServer, ApprovalResponse};
use crate::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};
use crate::persistence::approval_repo::ApprovalRepo;
use crate::persistence::session_repo::SessionRepo;
use crate::slack::blocks;
use crate::slack::client::SlackMessage;

/// Maximum number of diff lines to render inline in Slack.
const INLINE_DIFF_THRESHOLD: usize = 20;

/// Input parameters for the `ask_approval` tool per mcp-tools.json contract.
#[derive(Debug, serde::Deserialize)]
struct AskApprovalInput {
    /// Concise summary of the proposal.
    title: String,
    /// Contextual details about the proposed change.
    description: Option<String>,
    /// Standard unified diff or raw file content.
    diff: String,
    /// Target file path relative to `workspace_root`.
    file_path: String,
    /// Risk classification.
    #[serde(default = "default_risk_level")]
    risk_level: RiskLevel,
}

fn default_risk_level() -> RiskLevel {
    RiskLevel::Low
}

/// Handle the `ask_approval` tool call.
///
/// # Errors
///
/// Returns `rmcp::ErrorData` on validation or infrastructure failures.
#[allow(clippy::too_many_lines)] // Approval flow is inherently sequential with many steps.
pub async fn handle(
    context: ToolCallContext<'_, AgentRcServer>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let state = Arc::clone(context.service.state());
    let channel_id = context.service.effective_channel_id().map(str::to_owned);
    let args: serde_json::Map<String, serde_json::Value> = context.arguments.unwrap_or_default();

    let input: AskApprovalInput =
        serde_json::from_value(serde_json::Value::Object(args)).map_err(|err| {
            rmcp::ErrorData::invalid_params(format!("invalid ask_approval parameters: {err}"), None)
        })?;

    let span = info_span!(
        "ask_approval",
        file_path = %input.file_path,
        risk_level = ?input.risk_level,
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

        let workspace_root = std::path::PathBuf::from(&session.workspace_root);

        // ── Validate file path ───────────────────────────────
        let validated_path = crate::diff::validate_workspace_path(
            &workspace_root,
            &input.file_path,
        )
        .map_err(|err| {
            rmcp::ErrorData::invalid_params(format!("file path validation failed: {err}"), None)
        })?;

        // ── Compute SHA-256 hash of current file ─────────────
        let original_hash = super::util::compute_file_hash(&validated_path)
            .await
            .map_err(|err| {
                rmcp::ErrorData::internal_error(
                    format!("failed to read file for hash: {err}"),
                    None,
                )
            })?;

        // ── Create ApprovalRequest record ────────────────────
        let approval = ApprovalRequest::new(
            session.id.clone(),
            input.title.clone(),
            input.description.clone(),
            input.diff.clone(),
            input.file_path.clone(),
            input.risk_level,
            original_hash,
        );
        let request_id = approval.id.clone();

        let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));
        let _created = approval_repo.create(&approval).await.map_err(|err| {
            rmcp::ErrorData::internal_error(
                format!("failed to persist approval request: {err}"),
                None,
            )
        })?;

        // ── Post to Slack ────────────────────────────────────
        if let (Some(ref slack), Some(ref ch)) = (&state.slack, &channel_id) {
            let channel = SlackChannelId(ch.clone());
            let mut message_blocks = build_approval_blocks(
                &input.title,
                input.description.as_deref(),
                &input.diff,
                &input.file_path,
                input.risk_level,
            );
            message_blocks.push(blocks::approval_buttons(&request_id));

            let diff_line_count = input.diff.lines().count();

            if diff_line_count >= INLINE_DIFF_THRESHOLD {
                // Upload large diff as a file snippet.
                let upload_span = info_span!("slack_upload_diff", request_id = %request_id);
                async {
                    if let Err(err) = slack
                        .upload_file(
                            channel.clone(),
                            &format!("{}.diff", input.file_path.replace('/', "_")),
                            &input.diff,
                            None,
                        )
                        .await
                    {
                        warn!(%err, "failed to upload diff snippet to slack");
                    }
                }
                .instrument(upload_span)
                .await;
            }

            // Post the message with buttons.
            let post_span = info_span!("slack_post_approval", request_id = %request_id);
            async {
                let msg = SlackMessage {
                    channel,
                    text: Some(format!("\u{1f4cb} Approval Request: {}", input.title)),
                    blocks: Some(message_blocks),
                    thread_ts: None,
                };
                if let Err(err) = slack.enqueue(msg).await {
                    warn!(%err, "failed to enqueue approval message");
                }
            }
            .instrument(post_span)
            .await;
        } else {
            warn!("slack not configured; approval request will block without notification");
        }

        // ── Register oneshot and wait ────────────────────────
        let (tx, rx) = oneshot::channel::<ApprovalResponse>();
        {
            let mut pending = state.pending_approvals.lock().await;
            pending.insert(request_id.clone(), tx);
        }

        let timeout_seconds = state.config.timeouts.approval_seconds;
        let timeout_duration = Duration::from_secs(timeout_seconds);

        let response = tokio::time::timeout(timeout_duration, rx).await;

        let (status, reason) = match response {
            Ok(Ok(resp)) => (resp.status, resp.reason),
            Ok(Err(_)) => {
                // Sender dropped without sending (e.g., server shutdown).
                ("timeout".to_owned(), None)
            }
            Err(_elapsed) => {
                // Timeout expired — mark as expired and notify Slack.
                info!(
                    request_id = %request_id,
                    timeout_seconds,
                    "approval request timed out"
                );
                let _ = approval_repo
                    .update_status(&request_id, ApprovalStatus::Expired)
                    .await;

                if let (Some(ref slack), Some(ref ch)) = (&state.slack, &channel_id) {
                    let channel = SlackChannelId(ch.clone());
                    let msg = SlackMessage {
                        channel,
                        text: Some(format!(
                            "\u{23f1}\u{fe0f} Approval request '{}' timed out",
                            input.title
                        )),
                        blocks: Some(vec![blocks::severity_section(
                            "warning",
                            &format!(
                                "Approval request *{}* timed out after {} seconds",
                                input.title, timeout_seconds
                            ),
                        )]),
                        thread_ts: None,
                    };
                    let _ = slack.enqueue(msg).await;
                }

                ("timeout".to_owned(), None)
            }
        };

        // Clean up pending map.
        {
            let mut pending = state.pending_approvals.lock().await;
            pending.remove(&request_id);
        }

        // Update session last_tool.
        let _ = session_repo
            .update_last_activity(&session.id, Some("ask_approval".to_owned()))
            .await;

        info!(
            request_id = %request_id,
            status = %status,
            "ask_approval resolved"
        );

        // ── Build response ───────────────────────────────────
        let mut response_json = serde_json::json!({
            "status": status,
            "request_id": request_id,
        });
        if let Some(ref r) = reason {
            response_json["reason"] = serde_json::Value::String(r.clone());
        }

        Ok(CallToolResult::success(vec![rmcp::model::Content::json(
            response_json,
        )
        .map_err(|err| {
            rmcp::ErrorData::internal_error(
                format!("failed to serialize ask_approval response: {err}"),
                None,
            )
        })?]))
    }
    .instrument(span)
    .await
}

/// Build Slack Block Kit blocks for the approval message.
fn build_approval_blocks(
    title: &str,
    description: Option<&str>,
    diff: &str,
    file_path: &str,
    risk_level: RiskLevel,
) -> Vec<SlackBlock> {
    let mut result = Vec::new();

    // Header section.
    let risk_emoji = match risk_level {
        RiskLevel::Low => "\u{1f7e2}",
        RiskLevel::High => "\u{1f7e1}",
        RiskLevel::Critical => "\u{1f534}",
    };
    result.push(blocks::text_section(&format!(
        "{risk_emoji} *{title}*\n\u{1f4c4} `{file_path}` | Risk: *{risk_level:?}*"
    )));

    // Description, if provided.
    if let Some(desc) = description {
        result.push(blocks::text_section(desc));
    }

    // Inline diff for small changes.
    let diff_line_count = diff.lines().count();
    if diff_line_count < INLINE_DIFF_THRESHOLD {
        result.push(blocks::diff_section(diff));
    } else {
        result.push(blocks::text_section(&format!(
            "\u{1f4ce} Diff uploaded as file ({diff_line_count} lines)"
        )));
    }

    result
}
