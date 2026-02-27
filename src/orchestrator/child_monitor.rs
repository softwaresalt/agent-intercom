//! Child process monitor — detects unexpected agent process exits.
//!
//! Periodically polls all live child processes in the `ActiveChildren`
//! registry. When a process has exited, posts a notification to the
//! operator's Slack channel, removes it from the registry, and marks
//! the associated session as `Terminated`.

use std::sync::Arc;
use std::time::Duration;

use slack_morphism::prelude::SlackChannelId;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::mcp::handler::ActiveChildren;
use crate::models::session::SessionStatus;
use crate::persistence::session_repo::SessionRepo;
use crate::slack::client::{SlackMessage, SlackService};

/// Interval between polls for child process exits.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Spawn a background task that monitors spawned child processes for
/// unexpected exits and notifies the operator via Slack.
///
/// The task polls at [`POLL_INTERVAL`] until the `CancellationToken` fires.
///
/// # Arguments
///
/// * `children` — Shared registry of live child processes keyed by session ID.
/// * `slack`    — Slack service for posting notifications.
/// * `channel`  — Default Slack channel for crash notifications.
/// * `db`       — Database pool for marking sessions terminated.
/// * `cancel`   — Cancellation token for graceful shutdown.
#[must_use]
pub fn spawn_child_monitor(
    children: ActiveChildren,
    slack: Arc<SlackService>,
    channel: String,
    db: Arc<sqlx::SqlitePool>,
    cancel: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                () = cancel.cancelled() => {
                    info!("child process monitor shutting down");
                    break;
                }
                () = tokio::time::sleep(POLL_INTERVAL) => {}
            }

            poll_children(&children, &slack, &channel, &db).await;
        }
    })
}

/// Check all tracked children for unexpected exits. Any exited child is
/// removed from the registry, its session is terminated, and a Slack
/// notification is posted.
async fn poll_children(
    children: &Arc<Mutex<std::collections::HashMap<String, tokio::process::Child>>>,
    slack: &Arc<SlackService>,
    channel: &str,
    db: &Arc<sqlx::SqlitePool>,
) {
    let mut guard = children.lock().await;
    let mut exited: Vec<(String, Option<std::process::ExitStatus>)> = Vec::new();

    for (session_id, child) in guard.iter_mut() {
        match child.try_wait() {
            Ok(Some(status)) => {
                exited.push((session_id.clone(), Some(status)));
            }
            Ok(None) => {
                // Still running — skip.
            }
            Err(err) => {
                warn!(session_id, %err, "failed to poll child process status");
                // Treat as exited to clean up the dead entry.
                exited.push((session_id.clone(), None));
            }
        }
    }

    // Release the lock before doing async I/O.
    for (session_id, _) in &exited {
        guard.remove(session_id);
    }
    drop(guard);

    // Process each exited child.
    for (session_id, exit_status) in exited {
        let status_text = exit_status.map_or_else(
            || "status unknown".to_owned(),
            |s| {
                if s.success() {
                    "exited normally (code 0)".to_owned()
                } else {
                    s.code().map_or_else(
                        || "terminated by signal".to_owned(),
                        |c| format!("exited with code {c}"),
                    )
                }
            },
        );

        info!(session_id, status = %status_text, "spawned agent process exited");

        // Mark the session as Terminated in the database.
        let session_repo = SessionRepo::new(Arc::clone(db));
        if let Err(err) = session_repo
            .set_terminated(&session_id, SessionStatus::Terminated)
            .await
        {
            warn!(%err, session_id, "failed to terminate session after child exit");
        }

        // Post notification to Slack.
        let channel_id = SlackChannelId(channel.to_owned());
        let msg = SlackMessage::plain(
            channel_id,
            format!(
                "⚠️ Spawned agent process for session `{session_id}` has {status_text}. \
                 Session marked as terminated."
            ),
        );
        if let Err(err) = slack.enqueue(msg).await {
            warn!(%err, session_id, "failed to post child exit notification");
        }
    }
}
