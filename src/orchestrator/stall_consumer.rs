//! Stall event consumer — dispatches [`StallEvent`]s to Slack.
//!
//! Reads events from the shared `mpsc::Receiver<StallEvent>` channel and
//! posts formatted alert messages to the operator's Slack channel. The
//! consumer acknowledges all event variants: posts a stall alert with
//! action buttons on [`Stalled`], logs auto-nudge and escalation events,
//! and posts recovery confirmations on [`SelfRecovered`].
//!
//! When a session has a recorded `thread_ts` the alert is posted as a
//! threaded reply so it stays inside the session's dedicated Slack thread
//! (S037 / S038).
//!
//! # ACP nudge delivery (T097 / S064)
//!
//! For sessions running over the Agent Communication Protocol, auto-nudge
//! messages are delivered directly on the agent stream via
//! [`AgentDriver::send_prompt`] in addition to the Slack notification.
//! This ensures the agent can self-correct without requiring manual
//! operator intervention.

use std::sync::Arc;

use slack_morphism::prelude::{SlackChannelId, SlackTs};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::driver::AgentDriver;
use crate::models::session::ProtocolMode;
use crate::persistence::db::Database;
use crate::persistence::session_repo::SessionRepo;
use crate::slack::blocks;
use crate::slack::client::{SlackMessage, SlackService};

use super::stall_detector::StallEvent;

/// Spawn a background task that reads stall events and posts them to Slack.
///
/// The task runs until the `CancellationToken` fires or the `mpsc` channel
/// closes.  Returns a `JoinHandle` so the caller can await clean shutdown.
///
/// When a session has a recorded `channel_id` and `thread_ts` the alert is
/// posted to that session's dedicated Slack thread (S037).  Otherwise the
/// `channel` fallback is used with no thread anchor.
///
/// For ACP sessions, auto-nudge events are also delivered directly on the
/// agent stream via `driver` when provided (T097 / S064).
///
/// # Arguments
///
/// * `rx`      — Receiving end of the stall event channel.
/// * `slack`   — Slack service for posting messages.
/// * `channel` — Default Slack channel ID used when a session has no channel.
/// * `db`      — Database pool used to resolve session channel/thread context.
/// * `driver`  — Optional agent driver for ACP stream nudge delivery.
/// * `cancel`  — Cancellation token for graceful shutdown.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn spawn_stall_event_consumer(
    mut rx: mpsc::Receiver<StallEvent>,
    slack: Arc<SlackService>,
    channel: String,
    db: Arc<Database>,
    driver: Option<Arc<dyn AgentDriver>>,
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

            // Resolve session-scoped channel and thread_ts for threading (S037).
            let session_id_for_lookup = match &event {
                StallEvent::Stalled { session_id, .. }
                | StallEvent::AutoNudge { session_id, .. }
                | StallEvent::Escalated { session_id, .. }
                | StallEvent::SelfRecovered { session_id } => session_id.clone(),
            };

            let (effective_channel, thread_ts) =
                resolve_session_context(&session_id_for_lookup, &channel, &db).await;
            let channel_id = SlackChannelId(effective_channel);

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
                        thread_ts,
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

                    // T097 / S064: Deliver nudge directly on the ACP stream for
                    // ACP sessions so the agent can self-correct immediately.
                    deliver_acp_nudge_if_applicable(session_id, nudge_count, driver.as_ref(), &db)
                        .await;

                    let msg = SlackMessage {
                        channel: channel_id,
                        text: Some(format!(
                            "\u{1f514} Auto-nudge #{nudge_count} sent to session `{session_id}`"
                        )),
                        blocks: None,
                        thread_ts,
                    };
                    if let Err(err) = slack.enqueue(msg).await {
                        warn!(%err, "failed to post auto-nudge notification");
                    }
                }
                StallEvent::Escalated {
                    ref session_id,
                    nudge_count,
                } => {
                    warn!(session_id, nudge_count, "stall escalated");
                    let msg = SlackMessage {
                        channel: channel_id,
                        text: Some(format!(
                            "\u{1f6a8} *Stall escalated* \u{2014} session `{session_id}` exceeded \
                             {nudge_count} nudge attempts. Manual intervention required."
                        )),
                        blocks: None,
                        thread_ts,
                    };
                    if let Err(err) = slack.enqueue(msg).await {
                        warn!(%err, "failed to post escalation notification");
                    }
                }
                StallEvent::SelfRecovered { ref session_id } => {
                    info!(session_id, "agent self-recovered from stall");
                    let msg = SlackMessage {
                        channel: channel_id,
                        text: Some(format!(
                            "\u{2705} Agent in session `{session_id}` has self-recovered from stall"
                        )),
                        blocks: None,
                        thread_ts,
                    };
                    if let Err(err) = slack.enqueue(msg).await {
                        warn!(%err, "failed to post self-recovery notification");
                    }
                }
            }
        }
    })
}

/// Resolve the Slack channel and thread timestamp for a session.
///
/// Returns the session's `channel_id` (falling back to `default_channel`) and
/// its `thread_ts` (as `None` when not yet set) so stall alerts can be posted
/// to the correct Slack thread.
async fn resolve_session_context(
    session_id: &str,
    default_channel: &str,
    db: &Arc<Database>,
) -> (String, Option<SlackTs>) {
    let repo = SessionRepo::new(Arc::clone(db));
    match repo.get_by_id(session_id).await {
        Ok(Some(session)) => {
            let ch = session
                .channel_id
                .unwrap_or_else(|| default_channel.to_owned());
            let ts = session.thread_ts.map(SlackTs);
            (ch, ts)
        }
        Ok(None) => {
            warn!(session_id, "session not found for stall context lookup");
            (default_channel.to_owned(), None)
        }
        Err(err) => {
            warn!(%err, session_id, "failed to look up session for stall context");
            (default_channel.to_owned(), None)
        }
    }
}

/// Deliver an auto-nudge directly on the ACP agent stream for ACP sessions.
///
/// Looks up the session's `protocol_mode`; if ACP and a driver is available,
/// sends a `prompt/send` nudge message so the agent can self-correct without
/// requiring manual operator intervention (T097 / S064).
///
/// This is a best-effort operation: failures are logged but do not abort the
/// stall consumer loop.
async fn deliver_acp_nudge_if_applicable(
    session_id: &str,
    nudge_count: u32,
    driver: Option<&Arc<dyn AgentDriver>>,
    db: &Arc<Database>,
) {
    let Some(drv) = driver else { return };

    let repo = SessionRepo::new(Arc::clone(db));
    let session = match repo.get_by_id(session_id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            warn!(session_id, "session not found for ACP nudge delivery");
            return;
        }
        Err(err) => {
            warn!(%err, session_id, "failed to look up session for ACP nudge");
            return;
        }
    };

    if session.protocol_mode != ProtocolMode::Acp {
        return;
    }

    let nudge_text = format!(
        "You seem stalled. Auto-nudge #{nudge_count} — please continue working on your current task."
    );

    if let Err(err) = drv.send_prompt(session_id, &nudge_text).await {
        warn!(%err, session_id, nudge_count, "failed to deliver ACP nudge via driver stream");
    } else {
        info!(
            session_id,
            nudge_count, "ACP nudge delivered via driver stream"
        );
    }
}
