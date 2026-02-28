//! Slack interaction dispatch handler (T093, T094).
//!
//! Receives interactive payloads (button presses, modal submissions)
//! via Socket Mode. Applies a centralized authorization guard (FR-013,
//! SC-009) and double-submission prevention (FR-022) before dispatching
//! to the appropriate handler by `action_id` prefix.
//!
//! ## Authorization (T093)
//!
//! Every block action is checked against `authorized_user_ids` before
//! reaching any handler. Unauthorized attempts are silently ignored from
//! the Slack user's perspective but logged as security events.
//!
//! ## Double-Submission Prevention (T094)
//!
//! On first button action for any interactive message, the original
//! buttons are immediately replaced with a "Processing…" indicator via
//! `chat.update` *before* the handler executes. This guarantees at-most-
//! once semantics even if the handler is slow.

use std::sync::Arc;

use slack_morphism::prelude::{
    SlackBasicChannelInfo, SlackClient, SlackClientEventsUserState, SlackClientHyperHttpsConnector,
    SlackHistoryMessage, SlackInteractionEvent,
};
use tracing::{info, warn};

use crate::mcp::handler::AppState;
use crate::slack::{blocks, handlers};

// ── Centralized authorization check (T093 / FR-013, SC-009) ──────────

/// Verify that the acting Slack user is in the server's `authorized_user_ids`.
///
/// Returns `true` when authorized. On failure, logs a security event and
/// returns `false` — the caller should silently drop the interaction so
/// the unauthorized user receives no feedback beyond Slack's default
/// "interaction received" acknowledgment.
fn is_authorized(user_id: &str, state: &AppState) -> bool {
    if state
        .config
        .authorized_user_ids
        .iter()
        .any(|id| id == user_id)
    {
        return true;
    }

    // Log the security event per SC-009 but do NOT surface it to the user.
    warn!(
        user_id,
        "unauthorized user attempted slack interaction (silently ignored)"
    );
    false
}

// ── Double-submission prevention (T094 / FR-022) ─────────────────────

/// Replace interactive buttons with a transient "Processing…" indicator.
///
/// This runs *before* the handler so that any concurrent taps on the same
/// message are no-ops from the user's perspective.
async fn replace_buttons_with_processing(
    channel: Option<&SlackBasicChannelInfo>,
    message: Option<&SlackHistoryMessage>,
    state: &AppState,
) {
    let Some(ref slack) = state.slack else { return };
    let msg_ts = message.map(|m| m.origin.ts.clone());
    let chan_id = channel.map(|c| c.id.clone());

    if let (Some(ts), Some(ch)) = (msg_ts, chan_id) {
        let processing_blocks = vec![blocks::text_section("\u{23f3} Processing\u{2026}")];
        if let Err(err) = slack.update_message(ch, ts, processing_blocks).await {
            // Non-fatal — the handler will still attempt its own update.
            warn!(%err, "failed to apply double-submission guard");
        }
    }
}

/// Handle interactive payloads (buttons, modals) delivered via Socket Mode.
///
/// Applies a centralized authorization guard and double-submission
/// prevention before dispatching to the correct handler by `action_id`
/// prefix.
///
/// # Errors
///
/// Returns an error if the interaction cannot be processed.
#[allow(clippy::too_many_lines)] // Dispatch logic requires exhaustive match arms.
pub async fn handle_interaction(
    event: SlackInteractionEvent,
    _client: Arc<SlackClient<SlackClientHyperHttpsConnector>>,
    state: SlackClientEventsUserState,
) -> slack_morphism::UserCallbackResult<()> {
    // Extract shared AppState from the user state storage.
    let app_state: Option<Arc<AppState>> = {
        let guard = state.read().await;
        guard.get_user_state::<Arc<AppState>>().cloned()
    };

    match &event {
        SlackInteractionEvent::BlockActions(block_event) => {
            let user_id = block_event
                .user
                .as_ref()
                .map(|u| u.id.to_string())
                .unwrap_or_default();

            if user_id.is_empty() {
                warn!("block action with empty user ID; ignoring");
                return Ok(());
            }

            // ── T093: Centralized authorization guard ────────────
            // Check once at the dispatch level. Unauthorized users
            // are silently dropped with a security log.
            let Some(ref app) = app_state else {
                warn!("app state not available; cannot process interaction");
                return Ok(());
            };

            if !is_authorized(&user_id, app) {
                // Silent ignore per SC-009 — no error surfaced to Slack.
                return Ok(());
            }

            if let Some(actions) = &block_event.actions {
                // ── T094: Pre-dispatch double-submission guard ──
                // Replace buttons once before dispatching any actions.
                // This prevents concurrent taps from triggering the
                // handler a second time.
                //
                // Exception: actions that open a Slack modal must NOT
                // preemptively lock the buttons. If the operator dismisses
                // the modal without submitting, the original buttons must
                // remain clickable (FR-017). The ViewSubmission handler
                // replaces the buttons with a final status once the modal
                // is submitted.
                let opens_modal = actions.iter().any(|a| {
                    matches!(
                        a.action_id.to_string().as_str(),
                        "wait_resume_instruct" | "prompt_refine" | "approve_reject"
                    )
                });
                if !opens_modal {
                    replace_buttons_with_processing(
                        block_event.channel.as_ref(),
                        block_event.message.as_ref(),
                        app,
                    )
                    .await;
                }

                for action in actions {
                    let action_id = action.action_id.to_string();
                    info!(action_id, user_id, "dispatching block action");

                    // Route by action_id prefix to the correct handler.
                    if action_id.starts_with("approve_") {
                        if let Err(err) = handlers::approval::handle_approval_action(
                            action,
                            &user_id,
                            &block_event.trigger_id,
                            block_event.channel.as_ref(),
                            block_event.message.as_ref(),
                            app,
                        )
                        .await
                        {
                            warn!(%err, action_id, "approval action failed");
                        }
                    } else if action_id.starts_with("prompt_") {
                        if let Err(err) = handlers::prompt::handle_prompt_action(
                            action,
                            &user_id,
                            &block_event.trigger_id,
                            block_event.channel.as_ref(),
                            block_event.message.as_ref(),
                            app,
                        )
                        .await
                        {
                            warn!(%err, action_id, "prompt action failed");
                        }
                    } else if action_id.starts_with("stall_") {
                        if let Err(err) = handlers::nudge::handle_nudge_action(
                            action,
                            &user_id,
                            block_event.channel.as_ref(),
                            block_event.message.as_ref(),
                            app,
                        )
                        .await
                        {
                            warn!(%err, action_id, "nudge action failed");
                        }
                    } else if action_id.starts_with("wait_") {
                        if let Err(err) = handlers::wait::handle_wait_action(
                            action,
                            &user_id,
                            &block_event.trigger_id,
                            block_event.channel.as_ref(),
                            block_event.message.as_ref(),
                            app,
                        )
                        .await
                        {
                            warn!(%err, action_id, "wait action failed");
                        }
                    } else if action_id.starts_with("auto_approve_") {
                        if let Err(err) = handlers::command_approve::handle_auto_approve_action(
                            action,
                            &user_id,
                            block_event.channel.as_ref(),
                            block_event.message.as_ref(),
                            app,
                        )
                        .await
                        {
                            warn!(%err, action_id, "auto-approve action failed");
                        }
                    } else {
                        warn!(action_id, "unknown action_id prefix");
                    }
                }
            }
        }
        SlackInteractionEvent::ViewSubmission(view_event) => {
            let user_id = view_event.user.id.to_string();

            let Some(ref app) = app_state else {
                warn!("app state not available; cannot process view submission");
                return Ok(());
            };

            if !is_authorized(&user_id, app) {
                return Ok(());
            }

            info!(user_id, "dispatching view submission");

            if let Err(err) = handlers::modal::handle_view_submission(view_event, app).await {
                warn!(%err, "view submission handler failed");
            }
        }
        SlackInteractionEvent::ViewClosed(view_event) => {
            let user_id = view_event.user.id.to_string();

            let Some(ref app) = app_state else {
                return Ok(());
            };

            if !is_authorized(&user_id, app) {
                return Ok(());
            }

            // Extract the callback_id so we can clean up any cached modal context.
            let callback_id = match &view_event.view.view {
                slack_morphism::prelude::SlackView::Modal(modal) => modal
                    .callback_id
                    .as_ref()
                    .map(std::string::ToString::to_string)
                    .unwrap_or_default(),
                slack_morphism::prelude::SlackView::Home(_) => String::new(),
            };

            if callback_id.is_empty() {
                info!(user_id, "modal closed (no callback_id)");
            } else {
                let mut ctx = app.pending_modal_contexts.lock().await;
                let removed = ctx.remove(&callback_id).is_some();
                info!(
                    user_id,
                    callback_id, removed, "modal dismissed without submission"
                );
            }
        }
        _ => {
            info!(?event, "unhandled interaction event type");
        }
    }
    Ok(())
}
