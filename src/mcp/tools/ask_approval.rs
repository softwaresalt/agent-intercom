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

use crate::mcp::handler::{ApprovalResponse, IntercomServer};
use crate::models::approval::{ApprovalRequest, ApprovalStatus, RiskLevel};
use crate::persistence::approval_repo::ApprovalRepo;
use crate::persistence::session_repo::SessionRepo;
use crate::slack::blocks;
use crate::slack::client::SlackMessage;

/// Maximum number of diff lines to render inline in Slack.
const INLINE_DIFF_THRESHOLD: usize = 20;

/// A curated code excerpt supplied by the agent for operator review.
///
/// Snippets are posted as a threaded Slack reply using inline code blocks,
/// which Slack always renders as readable text (no content-scanner issues).
#[derive(Debug, serde::Deserialize)]
struct CodeSnippet {
    /// Short human-readable label (e.g. `"handle() — main entry point"`).
    label: String,
    /// Markdown code-fence language hint (e.g. `"rust"`, `"toml"`).  May be
    /// empty, in which case the code block is untagged.
    #[serde(default)]
    language: String,
    /// The code content to display.  Truncated server-side if it exceeds
    /// `SNIPPET_CHAR_LIMIT` characters.
    content: String,
}

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
    /// Curated code excerpts for inline Slack review.
    ///
    /// When provided, the agent identifies the most meaningful sections of
    /// the affected file (new functions, modified logic, key interfaces)
    /// and supplies them here.  The server posts these as a threaded Slack
    /// reply using inline code blocks so the operator can review them
    /// without opening any attachment.  When omitted, the server falls back
    /// to uploading the full original file as a Slack file attachment.
    #[serde(default)]
    snippets: Vec<CodeSnippet>,
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
    context: ToolCallContext<'_, IntercomServer>,
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
        // ── Early Slack channel check (T061 / S033) ─────────
        // Return a descriptive error instead of blocking indefinitely when
        // Slack is not configured or no channel_id is set for this session.
        if state.slack.is_none() || channel_id.is_none() {
            let (error_code, error_message) = if state.slack.is_none() {
                (
                    "slack_unavailable",
                    "Slack service is not configured; check_clearance requires Slack",
                )
            } else {
                (
                    "no_channel",
                    "no Slack channel configured for this session; \
                     set channel_id in the /mcp URL query string to enable approval requests",
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

        // ── Read original file content for Slack attachment ──
        // T084: Upload the existing file alongside the diff so operators
        // can review full context. Handles new-file and read-error cases
        // gracefully (T085, T086).
        let original_content =
            read_original_file_for_attachment(&validated_path, &original_hash).await;

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
                // Upload large diff as a file snippet.  Pass snippet_type
                // "text" so Slack pre-classifies the file before its content
                // scanner runs, preventing the "Binary" label.
                let upload_span = info_span!("slack_upload_diff", request_id = %request_id);
                async {
                    let sanitized = input.file_path.replace(['/', '.'], "_");
                    let filename = format!("{sanitized}.diff.txt");
                    if let Err(err) = slack
                        .upload_file(channel.clone(), &filename, &input.diff, None, Some("text"))
                        .await
                    {
                        warn!(%err, "failed to upload diff snippet to slack");
                    }
                }
                .instrument(upload_span)
                .await;
            }

            // Post the approval message directly so we can capture the Slack
            // `ts` for threading snippet replies.
            let post_span = info_span!("slack_post_approval", request_id = %request_id);
            let approval_ts: Option<slack_morphism::prelude::SlackTs> = async {
                let msg = SlackMessage {
                    channel: channel.clone(),
                    text: Some(format!("\u{1f4cb} Approval Request: {}", input.title)),
                    blocks: Some(message_blocks),
                    thread_ts: None,
                };
                match slack.post_message_direct(msg).await {
                    Ok(ts) => Some(ts),
                    Err(err) => {
                        warn!(%err, "failed to post approval message");
                        None
                    }
                }
            }
            .instrument(post_span)
            .await;

            // ── Snippet thread (preferred) or file upload (fallback) ──────
            //
            // When the agent supplies curated `snippets`, post them as a
            // threaded Slack reply.  Inline code blocks in messages always
            // render as readable text — no content-scanner interference.
            //
            // When no snippets are provided, fall back to uploading the full
            // original file for operator review (T084–T086).
            if !input.snippets.is_empty() {
                if let Some(ref ts) = approval_ts {
                    let snippet_span = info_span!("slack_post_snippets", request_id = %request_id);
                    async {
                        let snippet_blocks = blocks::code_snippet_blocks(
                            &input
                                .snippets
                                .iter()
                                .map(|s| {
                                    (s.label.as_str(), s.language.as_str(), s.content.as_str())
                                })
                                .collect::<Vec<_>>(),
                        );
                        let msg = SlackMessage {
                            channel: channel.clone(),
                            text: Some("Code snippets for review".into()),
                            blocks: Some(snippet_blocks),
                            thread_ts: Some(ts.clone()),
                        };
                        if let Err(err) = slack.enqueue(msg).await {
                            warn!(%err, "failed to post snippet thread");
                        }
                    }
                    .instrument(snippet_span)
                    .await;
                }
            } else if let Some(ref original) = original_content {
                // Fallback: upload the full original file (T084). Skipped for
                // new files (T085) or unreadable files (T086).
                let orig_span = info_span!("slack_upload_original", request_id = %request_id);
                async {
                    let sanitized = input.file_path.replace(['/', '.'], "_");
                    let filename = format!("{sanitized}.original.txt");
                    let lang = crate::slack::commands::file_extension_language(&input.file_path);
                    if let Err(err) = slack
                        .upload_file(channel.clone(), &filename, original, None, Some(lang))
                        .await
                    {
                        warn!(%err, "failed to upload original file to slack");
                    }
                }
                .instrument(orig_span)
                .await;
            }
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

/// Read the original file content for uploading as a Slack attachment (T084-T086).
///
/// Returns `Some(content)` when the file exists and is readable.
/// Returns `None` when:
/// - `original_hash` is `"new_file"` (the proposal is for a brand-new file) — T085
/// - the file cannot be read for any reason (deleted, permissions, etc.) — T086
///   In the error case a `warn!` is emitted but the approval flow continues.
pub async fn read_original_file_for_attachment(
    path: &std::path::Path,
    original_hash: &str,
) -> Option<String> {
    // T085: New files have no original content to attach.
    if original_hash == "new_file" {
        return None;
    }

    // T084/T086: Attempt to read the file, falling back gracefully on error.
    match tokio::fs::read_to_string(path).await {
        Ok(content) => Some(content),
        Err(err) => {
            tracing::warn!(
                %err,
                path = %path.display(),
                "failed to read original file for slack attachment; \
                 approval will proceed without original content"
            );
            None
        }
    }
}
