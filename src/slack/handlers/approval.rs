//! Approval interaction handler (T039).
//!
//! Handles Accept and Reject button presses from Slack interactive messages
//! for approval requests. Verifies the acting user belongs to
//! `authorized_user_ids` (FR-013), updates the database, resolves the
//! blocking oneshot channel, and replaces interactive buttons with a
//! static status line (FR-022).

use std::sync::Arc;

use slack_morphism::prelude::{
    SlackBasicChannelInfo, SlackHistoryMessage, SlackInteractionActionInfo,
};
use tracing::{info, warn};

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
/// * `channel` — channel where the message lives (for `chat.update`).
/// * `message` — the original Slack message (for retrieving `ts`).
/// * `state` — shared application state.
///
/// # Errors
///
/// Returns an error string if processing fails.
pub async fn handle_approval_action(
    action: &SlackInteractionActionInfo,
    user_id: &str,
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
        (ApprovalStatus::Approved, None)
    } else if action_id == "approve_reject" {
        (
            ApprovalStatus::Rejected,
            Some("rejected by operator".to_owned()),
        )
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
