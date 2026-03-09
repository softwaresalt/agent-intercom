//! Wait-for-instruction interaction handler (T086).
//!
//! Handles Resume and Stop button presses from Slack wait messages.
//! When "Resume with Instructions" is pressed, opens a modal to collect
//! the operator's instruction text. Verifies the acting user belongs to
//! `authorized_user_ids` (FR-013), resolves the blocking oneshot channel,
//! and replaces interactive buttons with a static status line (FR-022).

use std::sync::Arc;

use slack_morphism::prelude::{
    SlackBasicChannelInfo, SlackHistoryMessage, SlackInteractionActionInfo, SlackTriggerId,
};
use tracing::{info, warn};

use crate::mcp::handler::AppState;
use crate::persistence::session_repo::SessionRepo;
use crate::slack::blocks;
use crate::slack::handlers::check_session_ownership;

/// Process a single wait button action from Slack.
///
/// # Arguments
///
/// * `action` — the `SlackInteractionActionInfo` containing `action_id` and
///   `value` (the `session_id`).
/// * `user_id` — Slack user ID of the person who clicked.
/// * `trigger_id` — the Slack trigger ID from the block action event,
///   needed to open a modal for "Resume with Instructions".
/// * `channel` — channel where the message lives (for `chat.update`).
/// * `message` — the original Slack message (for retrieving `ts`).
/// * `state` — shared application state.
///
/// # Errors
///
/// Returns an error string if processing fails.
#[allow(clippy::too_many_lines)] // Modal caching + F-16/F-17 fallback logic cannot be shortened further.
pub async fn handle_wait_action(
    action: &SlackInteractionActionInfo,
    user_id: &str,
    trigger_id: &SlackTriggerId,
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

    // ── T068c / FR-031: Verify session ownership ─────────
    // The action value is the session_id directly, so we can look up the
    // session and verify the acting user is the owner.
    {
        let session_repo = SessionRepo::new(Arc::clone(&state.db));
        if let Ok(Some(session)) = session_repo.get_by_id(session_id).await {
            if let Err(err) = check_session_ownership(&session, user_id) {
                warn!(
                    user_id,
                    session_id,
                    owner = %session.owner_user_id,
                    "wait action rejected: non-owner attempt (FR-031)"
                );
                return Err(err.to_string());
            }
        }
    }

    // ── Determine response from action_id ────────────────
    let (_status, instruction) = if action_id == "wait_resume" {
        ("resumed".to_owned(), None)
    } else if action_id == "wait_resume_instruct" {
        // Open a modal to collect instruction text from the operator.
        // The oneshot will be resolved when the modal is submitted
        // (handled by modal::handle_view_submission).
        if let Some(ref slack) = state.slack {
            let callback_id = format!("wait_instruct:{session_id}");

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
                "Instructions",
                "Type your instructions for the agent\u{2026}",
            );
            if let Err(err) = slack.open_modal(trigger_id.clone(), modal).await {
                warn!(%err, session_id, "failed to open instruction modal; activating thread-reply fallback (F-16)");
                // Clean up cached context on failure.
                let mut ctx = state.pending_modal_contexts.lock().await;
                ctx.remove(&callback_id);
                // F-16/F-17: register thread-reply fallback when modal is unavailable.
                // Use the parent thread_ts (root of the Slack thread) as the map key so
                // that incoming replies, which report thread_ts = root, find the entry.
                // Falls back to origin.ts when the button message IS the thread root.
                let thread_ts_opt = message.map(|m| {
                    m.origin
                        .thread_ts
                        .as_ref()
                        .map_or_else(|| m.origin.ts.0.clone(), |ts| ts.0.clone())
                });
                let chan_id_opt = channel.map(|c| c.id.to_string());
                if let (Some(thread_ts), Some(chan_id)) = (thread_ts_opt, chan_id_opt) {
                    let button_msg_ts = message.map(|m| m.origin.ts.clone());
                    let state_clone = Arc::clone(state);
                    let session_id_owned = session_id.to_owned();
                    crate::slack::handlers::thread_reply::activate_thread_reply_fallback(
                        chan_id.as_str(),
                        thread_ts.as_str(),
                        session_id.to_owned(),
                        user_id.to_owned(),
                        "Modal unavailable \u{2014} please reply in this thread with your instructions.",
                        button_msg_ts,
                        slack,
                        Arc::clone(&state.pending_thread_replies),
                        session_id,
                        move |reply_text| async move {
                            if let Err(err) = state_clone
                                .driver
                                .resolve_wait(&session_id_owned, Some(reply_text))
                                .await
                            {
                                warn!(
                                    session_id = session_id_owned,
                                    %err,
                                    "thread-reply fallback: failed to resolve wait via driver"
                                );
                            }
                        },
                    )
                    .await?;
                    return Ok(());
                }
                return Err(format!("failed to open instruction modal: {err}"));
            }
        }
        // Return early — the oneshot is NOT resolved here; it will be
        // resolved when the ViewSubmission event arrives from the modal.
        return Ok(());
    } else if action_id == "wait_stop" {
        ("resumed".to_owned(), Some("stop".to_owned()))
    } else {
        return Err(format!("unknown wait action_id: {action_id}"));
    };

    info!(session_id, action_id, user_id, "wait action received");

    // ── Resolve oneshot channel via driver ───────────────
    {
        if let Err(err) = state
            .driver
            .resolve_wait(session_id, instruction.clone())
            .await
        {
            warn!(session_id, %err, "failed to resolve wait oneshot");
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
