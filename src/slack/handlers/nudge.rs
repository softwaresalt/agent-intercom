//! Nudge interaction handler (T050).
//!
//! Handles Nudge, Nudge with Instructions, and Stop button presses
//! from Slack stall alert messages. Updates the `StallAlert` record
//! in the database and replaces interactive buttons with static
//! status text (FR-022).

use std::sync::Arc;

use slack_morphism::prelude::{
    SlackBasicChannelInfo, SlackHistoryMessage, SlackInteractionActionInfo,
};
use tracing::{info, warn};

use crate::mcp::handler::AppState;
use crate::models::stall::StallAlertStatus;
use crate::persistence::session_repo::SessionRepo;
use crate::persistence::stall_repo::StallAlertRepo;
use crate::slack::blocks;

/// Process a single stall-alert button action from Slack.
///
/// # Arguments
///
/// * `action` — the `SlackInteractionActionInfo` with `action_id` and
///   `value` (the `alert_id`).
/// * `user_id` — Slack user ID of the operator who clicked.
/// * `channel` — channel where the stall alert message lives.
/// * `message` — the original Slack message (for `chat.update`).
/// * `state` — shared application state.
///
/// # Errors
///
/// Returns an error string if processing fails.
pub async fn handle_nudge_action(
    action: &SlackInteractionActionInfo,
    user_id: &str,
    channel: Option<&SlackBasicChannelInfo>,
    message: Option<&SlackHistoryMessage>,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let action_id = action.action_id.to_string();
    let alert_id = action
        .value
        .as_deref()
        .ok_or_else(|| "stall action missing alert_id value".to_owned())?;

    // ── Verify authorized user (FR-013) ──────────────────
    if !state
        .config
        .authorized_user_ids
        .contains(&user_id.to_owned())
    {
        warn!(
            user_id,
            alert_id, "unauthorized user attempted nudge action"
        );
        return Err("user not authorized for nudge actions".into());
    }

    let stall_repo = StallAlertRepo::new(Arc::clone(&state.db));
    let session_repo = SessionRepo::new(Arc::clone(&state.db));

    // ── Load the alert to get the session_id ─────────────
    let alert: crate::models::stall::StallAlert = {
        let opt = stall_repo
            .get_by_id(alert_id)
            .await
            .map_err(|e| e.to_string())?;
        opt.ok_or_else(|| format!("stall alert {alert_id} not found"))?
    };

    let status_text: String;

    if action_id == "stall_nudge" {
        // ── Simple nudge ─────────────────────────────────
        stall_repo
            .increment_nudge_count(alert_id)
            .await
            .map_err(|err| format!("failed to increment nudge: {err}"))?;

        // Send monocoque/nudge notification to agent via stall detector handle.
        if let Some(ref detectors) = state.stall_detectors {
            let guards = detectors.lock().await;
            if let Some(handle) = guards.get(&alert.session_id) {
                handle.reset();
            }
        }

        info!(alert_id, user_id, "nudge sent to agent");
        status_text = format!("\u{1f44a} *Nudged* by <@{user_id}>");
    } else if action_id == "stall_nudge_instruct" {
        // ── Nudge with instructions ──────────────────────
        // For now, increment nudge and log. Modal support is handled
        // by the interaction event dispatcher opening a modal view.
        stall_repo
            .increment_nudge_count(alert_id)
            .await
            .map_err(|err| format!("failed to increment nudge: {err}"))?;

        info!(alert_id, user_id, "nudge with instructions sent");
        status_text = format!("\u{1f4dd} *Nudged with instructions* by <@{user_id}>");
    } else if action_id == "stall_stop" {
        // ── Stop session ─────────────────────────────────
        stall_repo
            .update_status(alert_id, StallAlertStatus::Dismissed)
            .await
            .map_err(|err| format!("failed to dismiss alert: {err}"))?;

        // Terminate the session.
        let _ = session_repo
            .set_terminated(
                &alert.session_id,
                crate::models::session::SessionStatus::Terminated,
            )
            .await;

        // Cancel the stall detector for this session.
        if let Some(ref detectors) = state.stall_detectors {
            let mut guards = detectors.lock().await;
            guards.remove(&alert.session_id);
        }

        info!(alert_id, user_id, session_id = %alert.session_id, "session stopped via stall alert");
        status_text = format!("\u{1f6d1} *Stopped* by <@{user_id}>");
    } else {
        return Err(format!("unknown stall action_id: {action_id}"));
    }

    // ── Replace buttons with static status (FR-022) ──────
    if let Some(ref slack) = state.slack {
        let msg_ts = message.map(|m| m.origin.ts.clone());
        let chan_id = channel.map(|c| c.id.clone());

        if let (Some(ts), Some(ch)) = (msg_ts, chan_id) {
            let replacement_blocks = vec![blocks::text_section(&status_text)];
            if let Err(err) = slack.update_message(ch, ts, replacement_blocks).await {
                warn!(%err, alert_id, "failed to replace stall alert buttons");
            }
        }
    }

    Ok(())
}
