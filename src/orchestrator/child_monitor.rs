//! Child process monitor — detects unexpected agent process exits and drives
//! crash recovery.
//!
//! Periodically polls all live child processes in the `ActiveChildren`
//! registry. A process that exits normally (code 0) is treated as finished:
//! its session is marked `Terminated` and the operator is notified. A process
//! that exits abnormally (non-zero code, signal, or unknown status) is treated
//! as a crash: the session is respawned via [`spawner::respawn_session`], the
//! replacement child is registered for continued monitoring, and the operator
//! is notified. Respawns are bounded per crash chain by [`MAX_RESPAWN_ATTEMPTS`]
//! to prevent crash-loop storms.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use slack_morphism::prelude::SlackChannelId;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::config::GlobalConfig;
use crate::mcp::handler::ActiveChildren;
use crate::models::session::SessionStatus;
use crate::orchestrator::spawner;
use crate::persistence::session_repo::SessionRepo;
use crate::slack::client::{SlackMessage, SlackService};

/// Interval between polls for child process exits.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Maximum automatic respawns per crash chain before recovery is abandoned.
const MAX_RESPAWN_ATTEMPTS: u32 = 3;

/// Classification of a child process exit for crash-recovery decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitClass {
    /// Process exited normally with status code 0 — the agent finished.
    Clean,
    /// Process exited abnormally (non-zero code, killed by signal, or an
    /// unknown status from a failed poll). Treated as a crash to recover.
    Crash,
}

/// Classify a child process exit.
///
/// A successful exit (code 0) is [`ExitClass::Clean`]. Any other outcome —
/// including a missing status (`None`, e.g. a failed `try_wait`) — is
/// conservatively classified as [`ExitClass::Crash`] so recovery is attempted
/// rather than silently dropping a live session.
#[must_use]
pub fn classify_exit(status: Option<std::process::ExitStatus>) -> ExitClass {
    match status {
        Some(s) if s.success() => ExitClass::Clean,
        _ => ExitClass::Crash,
    }
}

/// Spawn a background task that monitors spawned child processes for
/// unexpected exits, recovering crashes and notifying the operator via Slack.
///
/// The task polls at [`POLL_INTERVAL`] until the `CancellationToken` fires.
///
/// # Arguments
///
/// * `children` — Shared registry of live child processes keyed by session ID.
/// * `slack`    — Slack service for posting notifications.
/// * `channel`  — Default Slack channel for crash notifications.
/// * `db`       — Database pool for session record updates.
/// * `config`   — Global configuration used to respawn crashed agents.
/// * `cancel`   — Cancellation token for graceful shutdown.
#[must_use]
pub fn spawn_child_monitor(
    children: ActiveChildren,
    slack: Arc<SlackService>,
    channel: String,
    db: Arc<sqlx::SqlitePool>,
    config: Arc<GlobalConfig>,
    cancel: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // Respawn counter keyed by crash-chain (propagated across restarts).
        let mut respawn_attempts: HashMap<String, u32> = HashMap::new();

        loop {
            tokio::select! {
                () = cancel.cancelled() => {
                    info!("child process monitor shutting down");
                    break;
                }
                () = tokio::time::sleep(POLL_INTERVAL) => {}
            }

            poll_children(
                &children,
                &slack,
                &channel,
                &db,
                &config,
                &mut respawn_attempts,
            )
            .await;
        }
    })
}

/// Check all tracked children for exits. Clean exits terminate the session;
/// crashes are routed to [`attempt_respawn`].
async fn poll_children(
    children: &ActiveChildren,
    slack: &Arc<SlackService>,
    channel: &str,
    db: &Arc<sqlx::SqlitePool>,
    config: &Arc<GlobalConfig>,
    respawn_attempts: &mut HashMap<String, u32>,
) {
    let mut guard = children.lock().await;
    let mut exited: Vec<(String, Option<std::process::ExitStatus>)> = Vec::new();

    for (session_id, child) in guard.iter_mut() {
        match child.try_wait() {
            Ok(Some(status)) => exited.push((session_id.clone(), Some(status))),
            Ok(None) => {
                // Still running — skip.
            }
            Err(err) => {
                warn!(session_id, %err, "failed to poll child process status");
                // Treat as exited (crash) to clean up and recover the entry.
                exited.push((session_id.clone(), None));
            }
        }
    }

    // Release the lock before doing async I/O.
    for (session_id, _) in &exited {
        guard.remove(session_id);
    }
    drop(guard);

    for (session_id, exit_status) in exited {
        match classify_exit(exit_status) {
            ExitClass::Clean => {
                info!(session_id, "spawned agent process exited normally (code 0)");
                let session_repo = SessionRepo::new(Arc::clone(db));
                if let Err(err) = session_repo
                    .set_terminated(&session_id, SessionStatus::Terminated)
                    .await
                {
                    warn!(%err, session_id, "failed to terminate session after clean exit");
                }
                respawn_attempts.remove(&session_id);
                notify(
                    slack,
                    channel,
                    format!(
                        "\u{2705} Spawned agent process for session `{session_id}` exited \
                         normally (code 0). Session marked as terminated."
                    ),
                )
                .await;
            }
            ExitClass::Crash => {
                attempt_respawn(
                    &session_id,
                    children,
                    slack,
                    channel,
                    db,
                    config,
                    respawn_attempts,
                )
                .await;
            }
        }
    }
}

/// Attempt to recover a crashed session by respawning it, bounded by
/// [`MAX_RESPAWN_ATTEMPTS`]. On success the resumed child is registered for
/// continued monitoring and the crash-chain counter is carried forward.
async fn attempt_respawn(
    session_id: &str,
    children: &ActiveChildren,
    slack: &Arc<SlackService>,
    channel: &str,
    db: &Arc<sqlx::SqlitePool>,
    config: &Arc<GlobalConfig>,
    respawn_attempts: &mut HashMap<String, u32>,
) {
    let attempts = respawn_attempts.remove(session_id).unwrap_or(0);
    let session_repo = SessionRepo::new(Arc::clone(db));

    if attempts >= MAX_RESPAWN_ATTEMPTS {
        warn!(
            session_id,
            attempts, "crash recovery exhausted; marking session interrupted"
        );
        if let Err(err) = session_repo
            .set_terminated(session_id, SessionStatus::Interrupted)
            .await
        {
            warn!(%err, session_id, "failed to mark exhausted session interrupted");
        }
        notify(
            slack,
            channel,
            format!(
                "\u{26d4} Agent session `{session_id}` crashed and exceeded the respawn \
                 limit ({MAX_RESPAWN_ATTEMPTS}). Marked interrupted — manual restart required."
            ),
        )
        .await;
        return;
    }

    let crashed = match session_repo.get_by_id(session_id).await {
        Ok(Some(session)) => session,
        Ok(None) => {
            warn!(
                session_id,
                "crashed session not found in db; cannot respawn"
            );
            notify(
                slack,
                channel,
                format!(
                    "\u{26a0}\u{fe0f} Agent session `{session_id}` crashed but its record could \
                     not be found; it cannot be automatically recovered."
                ),
            )
            .await;
            return;
        }
        Err(err) => {
            warn!(%err, session_id, "failed to load crashed session for respawn");
            // Best-effort: mark interrupted so the session is not left shown as
            // Active with no live child process.
            if let Err(e) = session_repo
                .set_terminated(session_id, SessionStatus::Interrupted)
                .await
            {
                warn!(%e, session_id, "failed to mark session interrupted after load error");
            }
            notify(
                slack,
                channel,
                format!(
                    "\u{26a0}\u{fe0f} Agent session `{session_id}` crashed and could not be loaded \
                     for recovery: {err}. Marked interrupted."
                ),
            )
            .await;
            return;
        }
    };

    match spawner::respawn_session(&crashed, config, &session_repo, db, config.http_port).await {
        Ok((resumed, child)) => {
            let resumed_id = resumed.id.clone();
            children.lock().await.insert(resumed_id.clone(), child);
            respawn_attempts.insert(resumed_id.clone(), attempts + 1);
            info!(session_id, resumed_id, "session respawned after crash");
            notify(
                slack,
                channel,
                format!(
                    "\u{1f504} Agent session `{session_id}` crashed and was automatically \
                     respawned as `{resumed_id}`. Resuming."
                ),
            )
            .await;
        }
        Err(err) => {
            warn!(%err, session_id, "failed to respawn crashed session");
            if let Err(e) = session_repo
                .set_terminated(session_id, SessionStatus::Interrupted)
                .await
            {
                warn!(%e, session_id, "failed to mark session interrupted after respawn failure");
            }
            notify(
                slack,
                channel,
                format!(
                    "\u{26a0}\u{fe0f} Agent session `{session_id}` crashed and could not be \
                     respawned: {err}. Marked interrupted."
                ),
            )
            .await;
        }
    }
}

/// Enqueue a plain-text notification to the operator channel, logging on failure.
async fn notify(slack: &Arc<SlackService>, channel: &str, text: String) {
    let msg = SlackMessage::plain(SlackChannelId(channel.to_owned()), text);
    if let Err(err) = slack.enqueue(msg).await {
        warn!(%err, "failed to post child monitor notification");
    }
}
