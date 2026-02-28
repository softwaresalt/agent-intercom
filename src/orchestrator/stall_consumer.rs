//! Stall event consumer — dispatches [`StallEvent`]s to Slack.
//!
//! Reads events from the shared `mpsc::Receiver<StallEvent>` channel and
//! posts formatted alert messages to the operator's Slack channel. The
//! consumer acknowledges all event variants: posts a stall alert with
//! action buttons on [`Stalled`], logs auto-nudge and escalation events,
//! and posts recovery confirmations on [`SelfRecovered`].

use std::sync::Arc;

use slack_morphism::prelude::SlackChannelId;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::slack::blocks;
use crate::slack::client::{SlackMessage, SlackService};

use super::stall_detector::StallEvent;

/// Spawn a background task that reads stall events and posts them to Slack.
///
/// The task runs until the `CancellationToken` fires or the `mpsc` channel
/// closes. Returns a `JoinHandle` so the caller can await clean shutdown.
///
/// # Arguments
///
/// * `rx`      — Receiving end of the stall event channel.
/// * `slack`   — Slack service for posting messages.
/// * `channel` — Default Slack channel ID for stall notifications.
/// * `cancel`  — Cancellation token for graceful shutdown.
#[must_use]
pub fn spawn_stall_event_consumer(
    mut rx: mpsc::Receiver<StallEvent>,
    slack: Arc<SlackService>,
    channel: String,
    cancel: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let event = tokio::select! {
                () = cancel.cancelled() => {
                    info!("stall event consumer shutting down");
                    break;
                }
                maybe_event = rx.recv() => {
                    if let Some(e) = maybe_event { e } else {
                        info!("stall event channel closed");
                        break;
                    }
                }
            };

            let channel_id = SlackChannelId(channel.clone());

            match event {
                StallEvent::Stalled {
                    ref session_id,
                    idle_seconds,
                } => {
                    info!(session_id, idle_seconds, "posting stall alert to slack");
                    let alert_blocks = blocks::stall_alert_blocks(session_id, idle_seconds);
                    let msg = SlackMessage {
                        channel: channel_id,
                        text: Some(format!(
                            "Agent stalled — session {session_id} idle for {idle_seconds}s"
                        )),
                        blocks: Some(alert_blocks),
                        thread_ts: None,
                    };
                    if let Err(err) = slack.enqueue(msg).await {
                        warn!(%err, "failed to post stall alert to slack");
                    }
                }
                StallEvent::AutoNudge {
                    ref session_id,
                    nudge_count,
                } => {
                    info!(session_id, nudge_count, "auto-nudge event");
                    let msg = SlackMessage::plain(
                        channel_id,
                        format!("🔔 Auto-nudge #{nudge_count} sent to session `{session_id}`"),
                    );
                    if let Err(err) = slack.enqueue(msg).await {
                        warn!(%err, "failed to post auto-nudge notification");
                    }
                }
                StallEvent::Escalated {
                    ref session_id,
                    nudge_count,
                } => {
                    warn!(session_id, nudge_count, "stall escalated");
                    let msg = SlackMessage::plain(
                        channel_id,
                        format!(
                            "🚨 *Stall escalated* — session `{session_id}` exceeded \
                             {nudge_count} nudge attempts. Manual intervention required."
                        ),
                    );
                    if let Err(err) = slack.enqueue(msg).await {
                        warn!(%err, "failed to post escalation notification");
                    }
                }
                StallEvent::SelfRecovered { ref session_id } => {
                    info!(session_id, "agent self-recovered from stall");
                    let msg = SlackMessage::plain(
                        channel_id,
                        format!("✅ Agent in session `{session_id}` has self-recovered from stall"),
                    );
                    if let Err(err) = slack.enqueue(msg).await {
                        warn!(%err, "failed to post self-recovery notification");
                    }
                }
            }
        }
    })
}
