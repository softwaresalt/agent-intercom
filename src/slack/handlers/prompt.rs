//! Prompt interaction handler (T058).
//!
//! Handles Continue, Refine, and Stop button presses from Slack
//! forwarded prompt messages. Verifies the acting user belongs to
//! `authorized_user_ids` (FR-013), updates the database, resolves the
//! blocking oneshot channel, and replaces interactive buttons with a
//! static status line (FR-022).

use std::sync::Arc;

use slack_morphism::prelude::{
    SlackBasicChannelInfo, SlackHistoryMessage, SlackInteractionActionInfo, SlackTriggerId,
};
use tracing::{info, warn};

use crate::mcp::handler::AppState;
use crate::models::prompt::PromptDecision;
use crate::persistence::prompt_repo::PromptRepo;
use crate::slack::blocks;

/// Process a single prompt button action from Slack.
///
/// # Arguments
///
/// * `action` — the `SlackInteractionActionInfo` containing `action_id` and
///   `value` (the `prompt_id`).
/// * `user_id` — Slack user ID of the person who clicked.
/// * `trigger_id` — the Slack trigger ID from the block action event,
///   needed to open a modal for "Refine".
/// * `channel` — channel where the message lives (for `chat.update`).
/// * `message` — the original Slack message (for retrieving `ts`).
/// * `state` — shared application state.
///
/// # Errors
///
/// Returns an error string if processing fails.
#[allow(clippy::too_many_lines)] // Dispatch + modal caching requires verbose match arms.
pub async fn handle_prompt_action(
    action: &SlackInteractionActionInfo,
    user_id: &str,
    trigger_id: &SlackTriggerId,
    channel: Option<&SlackBasicChannelInfo>,
    message: Option<&SlackHistoryMessage>,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let action_id = action.action_id.to_string();
    let prompt_id = action
        .value
        .as_deref()
        .ok_or_else(|| "prompt action missing prompt_id value".to_owned())?;

    // ── Verify authorised user (FR-013) ──────────────────
    if !state
        .config
        .authorized_user_ids
        .contains(&user_id.to_owned())
    {
        warn!(
            user_id,
            prompt_id, "unauthorised user attempted prompt action"
        );
        return Err("user not authorised for prompt actions".into());
    }

    // ── Determine decision from action_id ────────────────
    let (decision, instruction) = if action_id == "prompt_continue" {
        (PromptDecision::Continue, None)
    } else if action_id == "prompt_refine" {
        // Open a modal to collect instruction text from the operator.
        // The oneshot will be resolved when the modal is submitted
        // (handled by modal::handle_view_submission).
        if let Some(ref slack) = state.slack {
            let callback_id = format!("prompt_refine:{prompt_id}");

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
                "Refine",
                "Type your revised instructions\u{2026}",
            );
            if let Err(err) = slack.open_modal(trigger_id.clone(), modal).await {
                warn!(%err, prompt_id, "failed to open refine modal");
                // Clean up cached context on failure.
                let mut ctx = state.pending_modal_contexts.lock().await;
                ctx.remove(&callback_id);
                return Err(format!("failed to open refine modal: {err}"));
            }
        }
        // Return early — the oneshot is NOT resolved here; it will be
        // resolved when the ViewSubmission event arrives from the modal.
        return Ok(());
    } else if action_id == "prompt_stop" {
        (PromptDecision::Stop, None)
    } else {
        return Err(format!("unknown prompt action_id: {action_id}"));
    };

    // ── Update DB record ─────────────────────────────────
    let prompt_repo = PromptRepo::new(Arc::clone(&state.db));
    prompt_repo
        .update_decision(prompt_id, decision, instruction.clone())
        .await
        .map_err(|err| format!("failed to update prompt decision: {err}"))?;

    info!(prompt_id, ?decision, user_id, "prompt decision recorded");

    // ── Resolve oneshot channel via driver ───────────────
    {
        let decision_str = match decision {
            PromptDecision::Continue => "continue",
            PromptDecision::Refine => "refine",
            PromptDecision::Stop => "stop",
        };
        if let Err(err) = state
            .driver
            .resolve_prompt(prompt_id, decision_str, instruction)
            .await
        {
            warn!(prompt_id, %err, "failed to resolve prompt oneshot");
        }
    }

    // ── Replace buttons with static status (FR-022) ──────
    if let Some(ref slack) = state.slack {
        let status_text = match decision {
            PromptDecision::Continue => {
                format!("\u{25b6}\u{fe0f} *Continue* selected by <@{user_id}>")
            }
            PromptDecision::Refine => {
                format!("\u{270f}\u{fe0f} *Refine* selected by <@{user_id}>")
            }
            PromptDecision::Stop => {
                format!("\u{23f9}\u{fe0f} *Stop* selected by <@{user_id}>")
            }
        };

        // Get the message ts and channel for chat.update.
        let msg_ts = message.map(|m| m.origin.ts.clone());
        let chan_id = channel.map(|c| c.id.clone());

        if let (Some(ts), Some(ch)) = (msg_ts, chan_id) {
            let replacement_blocks = vec![blocks::text_section(&status_text)];
            if let Err(err) = slack.update_message(ch, ts, replacement_blocks).await {
                warn!(%err, prompt_id, "failed to replace prompt buttons");
            }
        } else {
            warn!(
                prompt_id,
                "missing message ts or channel; cannot replace buttons"
            );
        }
    }

    Ok(())
}
