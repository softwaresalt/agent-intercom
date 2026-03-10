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
use crate::persistence::session_repo::SessionRepo;
use crate::slack::blocks;
use crate::slack::handlers::check_session_ownership;

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

    // ── T068c / FR-031: Verify session ownership ─────────
    // Look up the prompt record to find its session, then confirm the
    // acting user is the session owner. Also capture session_id here for
    // thread-reply fallback registration (F-20 cleanup on termination).
    let prompt_session_id: String = {
        let prompt_repo = PromptRepo::new(Arc::clone(&state.db));
        if let Ok(Some(record)) = prompt_repo.get_by_id(prompt_id).await {
            let session_repo = SessionRepo::new(Arc::clone(&state.db));
            if let Ok(Some(session)) = session_repo.get_by_id(&record.session_id).await {
                if let Err(err) = check_session_ownership(&session, user_id) {
                    warn!(
                        user_id,
                        prompt_id,
                        owner = %session.owner_user_id,
                        "prompt action rejected: non-owner attempt (FR-031)"
                    );
                    return Err(err.to_string());
                }
            }
            record.session_id
        } else {
            String::new()
        }
    };

    // ── Determine decision from action_id ────────────────
    let (decision, instruction) = if action_id == "prompt_continue" {
        (PromptDecision::Continue, None)
    } else if action_id == "prompt_refine" {
        // Open a modal to collect instruction text from the operator.
        // The oneshot will be resolved when the modal is submitted
        // (handled by modal::handle_view_submission).
        if let Some(ref slack) = state.slack {
            let callback_id = format!("prompt_refine:{prompt_id}");

            // F-16/F-17 proactive: Slack silently suppresses views.open when the
            // triggering message lives inside a thread (origin.thread_ts is Some).
            // Skip the doomed views.open call and activate the fallback immediately,
            // telling the operator to use @agent-intercom instead of a modal.
            let is_thread_context = message.is_some_and(|m| m.origin.thread_ts.is_some());
            if is_thread_context {
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
                    let prompt_id_owned = prompt_id.to_owned();
                    crate::slack::handlers::thread_reply::activate_thread_reply_fallback(
                        chan_id.as_str(),
                        thread_ts.as_str(),
                        prompt_session_id.clone(),
                        user_id.to_owned(),
                        "Please tag `@agent-intercom` in this thread with your revised instructions.",
                        button_msg_ts,
                        slack,
                        Arc::clone(&state.pending_thread_replies),
                        prompt_id,
                        move |reply_text| async move {
                            let repo = PromptRepo::new(Arc::clone(&state_clone.db));
                            if let Err(db_err) = repo
                                .update_decision(
                                    &prompt_id_owned,
                                    PromptDecision::Refine,
                                    Some(reply_text.clone()),
                                )
                                .await
                            {
                                warn!(
                                    prompt_id = prompt_id_owned,
                                    %db_err,
                                    "thread-reply fallback: failed to update prompt decision in DB"
                                );
                            }
                            if let Err(driver_err) = state_clone
                                .driver
                                .resolve_prompt(&prompt_id_owned, "refine", Some(reply_text))
                                .await
                            {
                                warn!(
                                    prompt_id = prompt_id_owned,
                                    %driver_err,
                                    "thread-reply fallback: failed to resolve prompt via driver"
                                );
                            }
                        },
                    )
                    .await?;
                    return Ok(());
                }
                return Err(
                    "thread context: missing channel or thread_ts for Refine fallback".to_owned(),
                );
            }

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
                warn!(%err, prompt_id, "failed to open refine modal; activating thread-reply fallback (F-16)");
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
                    let prompt_id_owned = prompt_id.to_owned();
                    crate::slack::handlers::thread_reply::activate_thread_reply_fallback(
                        chan_id.as_str(),
                        thread_ts.as_str(),
                        prompt_session_id.clone(),
                        user_id.to_owned(),
                        "Modal unavailable \u{2014} please tag `@agent-intercom` in this thread with your revised instructions.",
                        button_msg_ts,
                        slack,
                        Arc::clone(&state.pending_thread_replies),
                        prompt_id,
                        move |reply_text| async move {
                            let repo = PromptRepo::new(Arc::clone(&state_clone.db));
                            if let Err(db_err) = repo
                                .update_decision(
                                    &prompt_id_owned,
                                    PromptDecision::Refine,
                                    Some(reply_text.clone()),
                                )
                                .await
                            {
                                warn!(
                                    prompt_id = prompt_id_owned,
                                    %db_err,
                                    "thread-reply fallback: failed to update prompt decision in DB"
                                );
                            }
                            if let Err(driver_err) = state_clone
                                .driver
                                .resolve_prompt(&prompt_id_owned, "refine", Some(reply_text))
                                .await
                            {
                                warn!(
                                    prompt_id = prompt_id_owned,
                                    %driver_err,
                                    "thread-reply fallback: failed to resolve prompt via driver"
                                );
                            }
                        },
                    )
                    .await?;
                    return Ok(());
                }
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
