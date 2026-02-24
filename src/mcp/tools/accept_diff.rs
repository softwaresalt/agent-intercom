//! `accept_diff` MCP tool handler (T045, T046).
//!
//! Applies previously approved code changes to the local file system.
//! Validates approval status, checks file integrity via SHA-256 hash
//! comparison, and performs atomic writes.

use std::sync::Arc;

use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::CallToolResult;
use slack_morphism::prelude::SlackChannelId;
use tracing::{info, info_span, warn, Instrument};

use crate::diff::patcher::apply_patch;
use crate::diff::writer::write_full_file;
use crate::mcp::handler::IntercomServer;
use crate::models::approval::ApprovalStatus;
use crate::persistence::approval_repo::ApprovalRepo;
use crate::persistence::session_repo::SessionRepo;
use crate::slack::blocks;
use crate::slack::client::SlackMessage;

/// Input parameters for the `accept_diff` tool per mcp-tools.json contract.
#[derive(Debug, serde::Deserialize)]
struct AcceptDiffInput {
    /// Unique identifier of the approved proposal.
    request_id: String,
    /// When `true`, overwrite even if local content has diverged.
    #[serde(default)]
    force: bool,
}

/// Build an error response with the standard `accept_diff` error schema.
fn error_result(code: &str, message: &str) -> CallToolResult {
    let body = serde_json::json!({
        "status": "error",
        "error_code": code,
        "error_message": message,
    });
    // The json! macro produces values that always serialize successfully.
    CallToolResult::success(vec![rmcp::model::Content::json(body)
        .unwrap_or_else(|_| rmcp::model::Content::text(format!("{code}: {message}")))])
}

/// Handle the `accept_diff` tool call.
///
/// # Errors
///
/// Returns `rmcp::ErrorData` on infrastructure failures.
/// Returns tool-level error codes for domain validation failures.
#[allow(clippy::too_many_lines)] // Sequential validation + apply flow.
pub async fn handle(
    context: ToolCallContext<'_, IntercomServer>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let state = Arc::clone(context.service.state());
    let channel_id = context.service.effective_channel_id().map(str::to_owned);
    let args: serde_json::Map<String, serde_json::Value> = context.arguments.unwrap_or_default();

    let input: AcceptDiffInput =
        serde_json::from_value(serde_json::Value::Object(args)).map_err(|err| {
            rmcp::ErrorData::invalid_params(format!("invalid accept_diff parameters: {err}"), None)
        })?;

    let span = info_span!(
        "accept_diff",
        request_id = %input.request_id,
        force = input.force,
    );

    async move {
        let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));

        // ── Look up the approval request ─────────────────────
        let Some(approval) = approval_repo
            .get_by_id(&input.request_id)
            .await
            .map_err(|err| {
                rmcp::ErrorData::internal_error(format!("approval query failed: {err}"), None)
            })?
        else {
            return Ok(error_result(
                "request_not_found",
                "no approval request found with the given id",
            ));
        };

        // ── Validate status is Approved ──────────────────────
        if approval.status != ApprovalStatus::Approved {
            if approval.status == ApprovalStatus::Consumed {
                return Ok(error_result(
                    "already_consumed",
                    "approved diff has already been applied",
                ));
            }
            return Ok(error_result(
                "not_approved",
                &format!(
                    "approval request is in {:?} status, not approved",
                    approval.status
                ),
            ));
        }

        // ── Resolve session for workspace root ───────────────
        let session_repo = SessionRepo::new(Arc::clone(&state.db));
        let Some(session) = session_repo
            .get_by_id(&approval.session_id)
            .await
            .map_err(|err| {
                rmcp::ErrorData::internal_error(format!("session query failed: {err}"), None)
            })?
        else {
            return Err(rmcp::ErrorData::internal_error(
                "owning session not found",
                None,
            ));
        };
        let workspace_root = std::path::PathBuf::from(&session.workspace_root);

        // ── Validate file path ───────────────────────────────
        let Ok(validated_path) =
            crate::diff::validate_workspace_path(&workspace_root, &approval.file_path)
        else {
            return Ok(error_result(
                "path_violation",
                "file path escapes workspace root",
            ));
        };

        // ── Hash comparison (integrity check) ────────────────
        let current_hash = super::util::compute_file_hash(&validated_path)
            .await
            .map_err(|err| {
                rmcp::ErrorData::internal_error(
                    format!("failed to read file for hash: {err}"),
                    None,
                )
            })?;
        let hash_matches = current_hash == approval.original_hash;

        info!(
            original_hash = %approval.original_hash,
            current_hash = %current_hash,
            hash_matches,
            "file integrity check"
        );

        if !hash_matches && !input.force {
            // T059 / S029 — Post conflict alert to Slack before returning error.
            if let (Some(ref slack), Some(ref ch)) = (&state.slack, &channel_id) {
                let channel = SlackChannelId(ch.clone());
                let msg = SlackMessage {
                    channel,
                    text: Some(format!(
                        "\u{274c} Patch conflict: {} has changed since proposal",
                        approval.file_path
                    )),
                    blocks: Some(vec![blocks::diff_conflict_section(&approval.file_path)]),
                    thread_ts: None,
                };
                let _ = slack.enqueue(msg).await;
            }
            return Ok(error_result(
                "patch_conflict",
                "file content has changed since proposal was created",
            ));
        }

        if !hash_matches && input.force {
            warn!(
                request_id = %input.request_id,
                file_path = %approval.file_path,
                "applying diff with force — file content has diverged"
            );

            // Log force-apply warning to Slack.
            if let (Some(ref slack), Some(ref ch)) = (&state.slack, &channel_id) {
                let channel = SlackChannelId(ch.clone());
                let msg = SlackMessage {
                    channel,
                    text: Some(format!(
                        "\u{26a0}\u{fe0f} Force-applying diff to {} (content diverged)",
                        approval.file_path
                    )),
                    blocks: Some(vec![blocks::diff_force_warning_section(
                        &approval.file_path,
                    )]),
                    thread_ts: None,
                };
                let _ = slack.enqueue(msg).await;
            }
        }

        // ── Determine write mode and apply ───────────────────
        let is_unified_diff =
            approval.diff_content.starts_with("--- ") || approval.diff_content.starts_with("diff ");

        let write_result = if is_unified_diff {
            apply_patch(&validated_path, &approval.diff_content, &workspace_root)
        } else {
            write_full_file(&validated_path, &approval.diff_content, &workspace_root)
        };

        let summary = match write_result {
            Ok(s) => s,
            Err(err) => {
                return Ok(error_result(
                    "patch_conflict",
                    &format!("failed to apply changes: {err}"),
                ));
            }
        };

        // ── Mark as consumed ─────────────────────────────────
        if let Err(err) = approval_repo.mark_consumed(&input.request_id).await {
            warn!(%err, "failed to mark approval as consumed");
        }

        // ── Post confirmation to Slack ───────────────────────
        if let (Some(ref slack), Some(ref ch)) = (&state.slack, &channel_id) {
            let channel = SlackChannelId(ch.clone());
            let msg = SlackMessage {
                channel,
                text: Some(format!(
                    "\u{2705} Applied: {} ({} bytes)",
                    approval.file_path, summary.bytes_written
                )),
                blocks: Some(vec![blocks::diff_applied_section(
                    &approval.file_path,
                    summary.bytes_written,
                )]),
                thread_ts: None,
            };
            let _ = slack.enqueue(msg).await;
        }

        // ── Update session last_tool ─────────────────────────
        let _ = session_repo
            .update_last_activity(&session.id, Some("accept_diff".to_owned()))
            .await;

        info!(
            request_id = %input.request_id,
            file_path = %approval.file_path,
            bytes_written = summary.bytes_written,
            "accept_diff completed successfully"
        );

        // ── Build response ───────────────────────────────────
        let response = serde_json::json!({
            "status": "applied",
            "files_written": [{
                "path": approval.file_path,
                "bytes": summary.bytes_written,
            }],
        });

        Ok(CallToolResult::success(vec![rmcp::model::Content::json(
            response,
        )
        .map_err(|err| {
            rmcp::ErrorData::internal_error(
                format!("failed to serialize accept_diff response: {err}"),
                None,
            )
        })?]))
    }
    .instrument(span)
    .await
}
