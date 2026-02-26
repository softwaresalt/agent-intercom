//! Approval interaction handler (T039).
//!
//! Handles Accept and Reject button presses from Slack interactive messages
//! for approval requests. Verifies the acting user belongs to
//! `authorized_user_ids` (FR-013), updates the database, resolves the
//! blocking oneshot channel, and replaces interactive buttons with a
//! static status line (FR-022).

use std::sync::Arc;

use slack_morphism::prelude::{
    SlackBasicChannelInfo, SlackHistoryMessage, SlackInteractionActionInfo, SlackTriggerId,
};
use tracing::{info, warn};

use crate::audit::{AuditEntry, AuditEventType};
use crate::mcp::handler::{AppState, ApprovalResponse};
use crate::models::approval::ApprovalStatus;
use crate::persistence::approval_repo::ApprovalRepo;
use crate::slack::blocks;

/// Process a single approval button action from Slack.
///
/// # Arguments
///
/// * `action` — the `SlackInteractionActionInfo` containing `action_id` and
///   `value` (the `request_id`).
/// * `user_id` — Slack user ID of the person who clicked.
/// * `trigger_id` — Slack trigger ID for opening a modal on rejection.
/// * `channel` — channel where the message lives (for `chat.update`).
/// * `message` — the original Slack message (for retrieving `ts`).
/// * `state` — shared application state.
///
/// # Errors
///
/// Returns an error string if processing fails.
#[allow(clippy::too_many_lines)] // Audit logging + FR-022 button replacement cannot be shortened further.
pub async fn handle_approval_action(
    action: &SlackInteractionActionInfo,
    user_id: &str,
    trigger_id: &SlackTriggerId,
    channel: Option<&SlackBasicChannelInfo>,
    message: Option<&SlackHistoryMessage>,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let action_id = action.action_id.to_string();
    let request_id = action
        .value
        .as_deref()
        .ok_or_else(|| "approval action missing request_id value".to_owned())?;

    // ── Verify authorised user (FR-013) ──────────────────
    if !state
        .config
        .authorized_user_ids
        .contains(&user_id.to_owned())
    {
        warn!(
            user_id,
            request_id, "unauthorised user attempted approval action"
        );
        return Err("user not authorised for approval actions".into());
    }

    // ── Determine status from action_id ──────────────────
    let (status, reason) = if action_id == "approve_accept" {
        (ApprovalStatus::Approved, None::<String>)
    } else if action_id == "approve_reject" {
        // Open a modal to collect the rejection reason from the operator.
        // The oneshot is resolved when the modal is submitted
        // (handled by modal::handle_view_submission with source "approval_reject").
        if let Some(ref slack) = state.slack {
            let callback_id = format!("approval_reject:{request_id}");

            // Cache the original message coordinates so the ViewSubmission
            // handler can update the message from "⏳ Processing…" to a
            // final status line (FR-022).
            let msg_ts = message.map(|m| m.origin.ts.to_string());
            let chan_id = channel.map(|c| c.id.to_string());
            if let (Some(ts), Some(ch)) = (msg_ts, chan_id) {
                let mut ctx = state.pending_modal_contexts.lock().await;
                ctx.insert(callback_id.clone(), (ch, ts));
            }

            let modal = blocks::instruction_modal(
                &callback_id,
                "Rejection Reason",
                "Describe why this change is being rejected\u{2026}",
            );
            if let Err(err) = slack.open_modal(trigger_id.clone(), modal).await {
                warn!(%err, request_id, "failed to open rejection reason modal");
                // Clean up cached context on failure.
                let mut ctx = state.pending_modal_contexts.lock().await;
                ctx.remove(&callback_id);
                return Err(format!("failed to open rejection reason modal: {err}"));
            }
        }
        // Return early — the rejection is NOT finalised here; it will be
        // completed when the ViewSubmission event arrives from the modal.
        return Ok(());
    } else {
        return Err(format!("unknown approval action_id: {action_id}"));
    };

    // ── Update DB record ─────────────────────────────────
    let approval_repo = ApprovalRepo::new(Arc::clone(&state.db));
    approval_repo
        .update_status(request_id, status)
        .await
        .map_err(|err| format!("failed to update approval status: {err}"))?;

    info!(
        request_id,
        ?status,
        user_id,
        "approval request status updated"
    );

    // Audit-log the approval/rejection decision (T059).
    if let Some(ref logger) = state.audit_logger {
        let event_type = match status {
            ApprovalStatus::Approved => AuditEventType::Approval,
            _ => AuditEventType::Rejection,
        };
        let mut entry = AuditEntry::new(event_type)
            .with_request_id(request_id.to_owned())
            .with_operator(user_id.to_owned());
        if let Some(ref r) = reason {
            entry = entry.with_reason(r.clone());
        }
        if let Err(err) = logger.log_entry(entry) {
            warn!(%err, "audit log write failed (approval action)");
        }
    }

    // ── Resolve oneshot channel ──────────────────────────
    {
        let mut pending = state.pending_approvals.lock().await;
        if let Some(tx) = pending.remove(request_id) {
            let response = ApprovalResponse {
                status: match status {
                    ApprovalStatus::Approved => "approved".to_owned(),
                    ApprovalStatus::Rejected => "rejected".to_owned(),
                    _ => "unknown".to_owned(),
                },
                reason: reason.clone(),
            };
            if tx.send(response).is_err() {
                warn!(request_id, "oneshot receiver already dropped");
            }
        } else {
            warn!(
                request_id,
                "no pending oneshot found (request may have timed out)"
            );
        }
    }

    // ── Replace buttons with static status (FR-022) ──────
    if let Some(ref slack) = state.slack {
        let status_text = match status {
            ApprovalStatus::Approved => {
                format!("\u{2705} *Approved* by <@{user_id}>")
            }
            ApprovalStatus::Rejected => {
                let reason_text = reason.as_deref().unwrap_or("no reason given");
                format!("\u{274c} *Rejected* by <@{user_id}>: {reason_text}")
            }
            _ => format!("Status updated by <@{user_id}>"),
        };

        // Get the message ts and channel for chat.update.
        let msg_ts = message.map(|m| m.origin.ts.clone());
        let chan_id = channel.map(|c| c.id.clone());

        if let (Some(ts), Some(ch)) = (msg_ts, chan_id) {
            let replacement_blocks = vec![blocks::text_section(&status_text)];
            if let Err(err) = slack.update_message(ch, ts, replacement_blocks).await {
                warn!(%err, request_id, "failed to replace approval buttons");
            }
        } else {
            warn!(
                request_id,
                "missing message ts or channel; cannot replace buttons"
            );
        }
    }

    Ok(())
}
