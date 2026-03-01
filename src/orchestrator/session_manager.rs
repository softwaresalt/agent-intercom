//! Session lifecycle management: pause, resume, terminate.
//!
//! Provides high-level operations for controlling session state from
//! Slack slash commands or IPC. All operations validate session
//! ownership before proceeding (FR-013).

use std::time::Duration;

use slack_morphism::prelude::{SlackChannelId, SlackTs};
use tokio::process::Child;
use tracing::{info, info_span, warn};

use crate::models::session::{Session, SessionStatus};
use crate::persistence::session_repo::SessionRepo;
use crate::slack::client::{SlackMessage, SlackService};
use crate::{AppError, Result};

/// Pause a running session.
///
/// Sets the session status to `Paused` so that no further tool calls are
/// processed until [`resume_session`] is called.
///
/// # Errors
///
/// Returns `AppError::Db` if the status transition is invalid.
pub async fn pause_session(session_id: &str, session_repo: &SessionRepo) -> Result<Session> {
    let span = info_span!("pause_session", session_id);
    let _guard = span.enter();

    let session = session_repo
        .update_status(session_id, SessionStatus::Paused)
        .await?;

    info!(session_id, "session paused");
    Ok(session)
}

/// Resume a paused session.
///
/// Reactivates the session so tool calls are processed again.
///
/// # Errors
///
/// Returns `AppError::Db` if the status transition is invalid.
pub async fn resume_session(session_id: &str, session_repo: &SessionRepo) -> Result<Session> {
    let span = info_span!("resume_session", session_id);
    let _guard = span.enter();

    let session = session_repo
        .update_status(session_id, SessionStatus::Active)
        .await?;

    info!(session_id, "session resumed");
    Ok(session)
}

/// Terminate a session, killing the child process with a 5-second grace period.
///
/// Waits up to 5 seconds for the child process to exit on its own (it may
/// have already received an EOF on its stdin).  If the process has not
/// exited after the grace period, it is force-killed.  Updates the session
/// status to `Terminated` in the database.
///
/// # Errors
///
/// Returns `AppError::Db` if the status update fails, or `AppError::Mcp` if
/// the process termination encounters an error.
pub async fn terminate_session(
    session_id: &str,
    session_repo: &SessionRepo,
    child: Option<&mut Child>,
) -> Result<Session> {
    let span = info_span!("terminate_session", session_id);
    let _guard = span.enter();

    // Attempt graceful termination of the child process.
    if let Some(process) = child {
        info!(
            session_id,
            "waiting for child process to exit (5s grace period)"
        );

        // Wait up to 5s for a natural exit, then force-kill.  The child
        // process has `kill_on_drop(true)` set, so dropping it will also
        // terminate it, but we prefer an explicit flow here for logging.
        let grace = Duration::from_secs(5);
        let wait_result = tokio::time::timeout(grace, process.wait()).await;

        match wait_result {
            Ok(Ok(exit)) => {
                info!(session_id, ?exit, "child process exited gracefully");
            }
            Ok(Err(err)) => {
                warn!(session_id, %err, "error waiting for child process");
            }
            Err(_) => {
                // Timeout — force kill.
                warn!(
                    session_id,
                    "child process did not exit within grace period, forcing kill"
                );
                if let Err(err) = process.kill().await {
                    warn!(session_id, %err, "failed to force-kill child process");
                }
            }
        }
    }

    // Update session status in the database.
    let session = session_repo
        .set_terminated(session_id, SessionStatus::Terminated)
        .await?;

    info!(session_id, "session terminated");
    Ok(session)
}

/// Resolve the active session for a user, optionally by explicit ID.
///
/// If `session_id` is provided, looks up that specific session.
/// Otherwise, returns the most recently active session owned by the user.
///
/// # Errors
///
/// Returns `AppError::NotFound` if no matching session exists, or
/// `AppError::Unauthorized` if the user is not the session owner.
pub async fn resolve_session(
    session_id: Option<&str>,
    user_id: &str,
    session_repo: &SessionRepo,
) -> Result<Session> {
    let session = if let Some(id) = session_id {
        session_repo
            .get_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("session {id} not found")))?
    } else {
        // Find the most recently active session for this user.
        let active = session_repo.list_active().await?;
        active
            .into_iter()
            .find(|s| s.owner_user_id == user_id)
            .ok_or_else(|| AppError::NotFound("no active session found for user".into()))?
    };

    if session.owner_user_id != user_id {
        return Err(AppError::Unauthorized(
            "session belongs to a different operator".into(),
        ));
    }

    Ok(session)
}

/// Post a "Session ended" summary as a threaded Slack reply (T060 / S094 / S095).
///
/// If the session has both a `channel_id` and a `thread_ts`, the summary is
/// posted as a reply to the session's dedicated Slack thread so the operator
/// can see the final status in context.  When either field is absent the call
/// is a silent no-op.
///
/// # Arguments
///
/// * `session` — The terminated or interrupted session.
/// * `reason`  — Human-readable reason for the session ending.
/// * `slack`   — Slack service used to enqueue the message.
pub async fn notify_session_ended(session: &Session, reason: &str, slack: &SlackService) {
    let (Some(ref channel_id), Some(ref thread_ts)) = (&session.channel_id, &session.thread_ts)
    else {
        return;
    };

    let ended_blocks = crate::slack::blocks::session_ended_blocks(session, reason);
    let msg = SlackMessage {
        channel: SlackChannelId(channel_id.clone()),
        text: Some(format!(
            "\u{1f3c1} Session `{}` ended",
            session.id.chars().take(8).collect::<String>()
        )),
        blocks: Some(ended_blocks),
        thread_ts: Some(SlackTs(thread_ts.clone())),
    };

    if let Err(err) = slack.enqueue(msg).await {
        warn!(%err, session_id = %session.id, "failed to post session-ended notification");
    } else {
        info!(session_id = %session.id, "posted session-ended summary to thread");
    }
}
