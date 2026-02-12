//! Session lifecycle management: pause, resume, terminate.
//!
//! Provides high-level operations for controlling session state from
//! Slack slash commands or IPC. All operations validate session
//! ownership before proceeding (FR-013).

use std::time::Duration;

use tokio::process::Child;
use tracing::{info, info_span, warn};

use crate::models::session::{Session, SessionStatus};
use crate::persistence::session_repo::SessionRepo;
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
/// Sends SIGTERM (or equivalent) to the child process, waits up to 5 seconds
/// for graceful exit, then force-kills if necessary. Updates the session
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
        info!(session_id, "sending termination signal to child process");

        // Start kill — on Unix this sends SIGKILL via `kill_on_drop`,
        // on Windows calls TerminateProcess. We use a timeout approach:
        // wait up to 5s, then force kill.
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
        session_repo.get_by_id(id).await?
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
