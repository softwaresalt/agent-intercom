//! Slack push event handler for app mentions and channel messages.
//!
//! Dispatches incoming push events (delivered via Socket Mode) to the
//! appropriate steering handler. App mentions and thread messages are
//! treated as operator steering input for the active session in the
//! originating channel.

use std::sync::Arc;

use slack_morphism::prelude::{
    SlackChannelId, SlackClient, SlackClientEventsUserState, SlackClientHyperHttpsConnector,
    SlackEventCallbackBody, SlackPushEventCallback, SlackTs,
};
use tracing::{debug, info, warn};

use crate::mcp::handler::AppState;
use crate::slack::{client::SlackMessage, handlers};

/// Handle push events (app mentions, channel messages) from Socket Mode.
///
/// The `SlackPushEventCallback` is the inner event callback — the framework
/// has already unwrapped the outer `SlackPushEvent` envelope.
///
/// # Errors
///
/// Returns `Ok(())` on success or if the event is silently ignored.
pub async fn handle_push_event(
    callback: SlackPushEventCallback,
    _client: Arc<SlackClient<SlackClientHyperHttpsConnector>>,
    state: SlackClientEventsUserState,
) -> slack_morphism::UserCallbackResult<()> {
    let app_state: Option<Arc<AppState>> = {
        let guard = state.read().await;
        guard.get_user_state::<Arc<AppState>>().cloned()
    };

    let Some(app) = app_state else {
        warn!("push event: app state not available");
        return Ok(());
    };

    match callback.event {
        SlackEventCallbackBody::AppMention(mention) => {
            let user_id = mention.user.to_string();

            if !is_authorized(&user_id, &app) {
                return Ok(());
            }

            let channel_id = mention.channel.to_string();
            let thread_ts = mention.origin.thread_ts.clone();
            let text = mention
                .content
                .text
                .as_deref()
                .unwrap_or_default();

            info!(
                user_id,
                channel_id,
                "push event: app mention received"
            );

            handlers::steer::ingest_app_mention(text, &channel_id, &app).await;
            post_ack(&app, &channel_id, thread_ts.as_ref()).await;
        }

        SlackEventCallbackBody::Message(msg) => {
            // Skip bot messages and subtypes (edits, deletes, joins, etc.)
            // to avoid echo loops and irrelevant noise.
            if msg.sender.bot_id.is_some() {
                debug!("push event: ignoring bot message");
                return Ok(());
            }
            if msg.subtype.is_some() {
                debug!("push event: ignoring message subtype");
                return Ok(());
            }

            let Some(ref user_id) = msg.sender.user else {
                debug!("push event: message with no user, ignoring");
                return Ok(());
            };
            let user_str = user_id.to_string();

            if !is_authorized(&user_str, &app) {
                return Ok(());
            }

            // Only process thread replies — top-level channel messages are
            // not steering input (those come via slash commands).
            let Some(ref thread_ts) = msg.origin.thread_ts else {
                debug!("push event: top-level message, not a thread reply — ignoring");
                return Ok(());
            };

            let Some(ref channel_id) = msg.origin.channel else {
                debug!("push event: message with no channel, ignoring");
                return Ok(());
            };
            let channel_str = channel_id.to_string();

            let text = msg
                .content
                .as_ref()
                .and_then(|c| c.text.as_deref())
                .unwrap_or_default()
                .trim();

            if text.is_empty() {
                debug!("push event: empty thread message, ignoring");
                return Ok(());
            }

            info!(
                user = user_str,
                channel = channel_str,
                "push event: thread message → steering"
            );

            match handlers::steer::store_from_slack(text, Some(&channel_str), &app).await {
                Ok(result) => {
                    info!(channel = channel_str, %result, "thread message → steer stored");
                    post_ack(&app, &channel_str, Some(thread_ts)).await;
                }
                Err(err) => {
                    warn!(channel = channel_str, %err, "thread message → steer failed");
                }
            }
        }

        other => {
            debug!(?other, "push event: unhandled event type, ignoring");
        }
    }

    Ok(())
}

/// Post a brief acknowledgment to the session thread so the operator knows
/// their message was received.
async fn post_ack(state: &AppState, channel_id: &str, thread_ts: Option<&SlackTs>) {
    let Some(ref slack) = state.slack else { return };
    let msg = SlackMessage {
        channel: SlackChannelId(channel_id.to_owned()),
        text: Some("\u{1f4e1} 10-4".to_owned()),
        blocks: None,
        thread_ts: thread_ts.cloned(),
    };
    if let Err(err) = slack.enqueue(msg).await {
        warn!(%err, "push event: failed to post ack");
    }
}

/// Check if the user is in the authorized list.
fn is_authorized(user_id: &str, state: &AppState) -> bool {
    if state
        .config
        .authorized_user_ids
        .iter()
        .any(|id| id == user_id)
    {
        return true;
    }
    warn!(
        user_id,
        "push event: unauthorized user (silently ignored)"
    );
    false
}
