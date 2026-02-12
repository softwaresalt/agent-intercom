//! Wait-for-instruction interaction handler (T086).
//!
//! Handles Resume and Stop button presses from Slack wait messages.
//! Verifies the acting user belongs to `authorized_user_ids` (FR-013),
//! resolves the blocking oneshot channel, and replaces interactive
//! buttons with a static status line (FR-022).

use std::sync::Arc;

use slack_morphism::prelude::{
    SlackBasicChannelInfo, SlackHistoryMessage, SlackInteractionActionInfo,
};
use tracing::{info, warn};

use crate::mcp::handler::{AppState, WaitResponse};
use crate::slack::blocks;

/// Process a single wait button action from Slack.
///
/// # Arguments
///
/// * `action` — the `SlackInteractionActionInfo` containing `action_id` and
///   `value` (the `session_id`).
/// * `user_id` — Slack user ID of the person who clicked.
/// * `channel` — channel where the message lives (for `chat.update`).
/// * `message` — the original Slack message (for retrieving `ts`).
/// * `state` — shared application state.
///
/// # Errors
///
/// Returns an error string if processing fails.
pub async fn handle_wait_action(
    action: &SlackInteractionActionInfo,
    user_id: &str,
    channel: Option<&SlackBasicChannelInfo>,
    message: Option<&SlackHistoryMessage>,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let action_id = action.action_id.to_string();
    let session_id = action
        .value
        .as_deref()
        .ok_or_else(|| "wait action missing session_id value".to_owned())?;

    // ── Verify authorised user (FR-013) ──────────────────
    if !state
        .config
        .authorized_user_ids
        .contains(&user_id.to_owned())
    {
        warn!(
            user_id,
            session_id, "unauthorised user attempted wait action"
        );
        return Err("user not authorised for wait actions".into());
    }

    // ── Determine response from action_id ────────────────
    let (status, instruction) = if action_id == "wait_resume" {
        ("resumed".to_owned(), None)
    } else if action_id == "wait_resume_instruct" {
        // For resume with instructions, use a placeholder;
        // modal support will be added in a future iteration.
        (
            "resumed".to_owned(),
            Some("(instruction via Slack)".to_owned()),
        )
    } else if action_id == "wait_stop" {
        ("resumed".to_owned(), Some("stop".to_owned()))
    } else {
        return Err(format!("unknown wait action_id: {action_id}"));
    };

    info!(session_id, action_id, user_id, "wait action received");

    // ── Resolve oneshot channel ──────────────────────────
    {
        let mut pending = state.pending_waits.lock().await;
        if let Some(tx) = pending.remove(session_id) {
            let response = WaitResponse {
                status,
                instruction: instruction.clone(),
            };
            if tx.send(response).is_err() {
                warn!(session_id, "wait oneshot receiver already dropped");
            }
        } else {
            warn!(
                session_id,
                "no pending wait oneshot found (may have timed out)"
            );
        }
    }

    // ── Replace buttons with static status (FR-022) ──────
    if let Some(ref slack) = state.slack {
        let status_text = if action_id == "wait_stop" {
            format!("\u{23f9}\u{fe0f} *Stop* requested by <@{user_id}>")
        } else {
            format!("\u{25b6}\u{fe0f} *Resumed* by <@{user_id}>")
        };

        let msg_ts = message.map(|m| m.origin.ts.clone());
        let chan_id = channel.map(|c| c.id.clone());

        if let (Some(ts), Some(ch)) = (msg_ts, chan_id) {
            let replacement_blocks = vec![blocks::text_section(&status_text)];
            if let Err(err) = slack.update_message(ch, ts, replacement_blocks).await {
                warn!(%err, session_id, "failed to replace wait buttons");
            }
        } else {
            warn!(
                session_id,
                "missing message ts or channel; cannot replace buttons"
            );
        }
    }

    Ok(())
}
