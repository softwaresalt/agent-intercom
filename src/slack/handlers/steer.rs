//! Operator steering message ingestion handler (T020).
//!
//! Provides shared logic for storing steering messages from Slack app
//! mentions, slash commands, and IPC requests. Messages are associated
//! with the active session for the originating channel and delivered
//! to the agent on the next `ping` call.

use std::sync::Arc;

use tracing::{info, warn};

use crate::mcp::handler::AppState;
use crate::models::steering::{SteeringMessage, SteeringSource};
use crate::persistence::session_repo::SessionRepo;
use crate::persistence::steering_repo::SteeringRepo;

/// Store a steering message from a Slack channel.
///
/// Looks up the active session for the given channel, then stores the
/// message with `source = Slack`. Returns an operator-visible string
/// confirming storage or describing the failure.
///
/// When `channel_id` is `Some`, only sessions associated with that channel
/// are considered (S043 / RI-04 fix). If no session is active in the
/// channel the caller receives a descriptive "no active session in this
/// channel" error (S045).
///
/// # Errors
///
/// Returns an `AppError` if session lookup or message insertion fails.
pub async fn store_from_slack(
    text: &str,
    channel_id: Option<&str>,
    state: &Arc<AppState>,
) -> crate::Result<String> {
    if text.trim().is_empty() {
        return Err(crate::AppError::Config(
            "steering message text cannot be empty".into(),
        ));
    }

    let session_repo = SessionRepo::new(Arc::clone(&state.db));

    let session = if let Some(ch) = channel_id {
        // T065 / S043: scope session lookup to the originating channel.
        let sessions = session_repo
            .find_active_by_channel(ch)
            .await
            .map_err(|err| {
                crate::AppError::Db(format!("failed to find sessions in channel: {err}"))
            })?;
        sessions.into_iter().next().ok_or_else(|| {
            crate::AppError::Config(
                "no active session in this channel — start a session first".into(),
            )
        })?
    } else {
        // No channel context: fall back to any active session (IPC / tests).
        let sessions = session_repo
            .list_active()
            .await
            .map_err(|err| crate::AppError::Db(format!("failed to list sessions: {err}")))?;
        sessions.into_iter().next().ok_or_else(|| {
            crate::AppError::Config("no active session to steer — start a session first".into())
        })?
    };

    let msg = SteeringMessage::new(
        session.id.clone(),
        channel_id.map(str::to_owned),
        text.to_owned(),
        SteeringSource::Slack,
    );

    let steering_repo = SteeringRepo::new(Arc::clone(&state.db));
    steering_repo.insert(&msg).await?;

    info!(
        session_id = %session.id,
        channel_id = ?channel_id,
        "steering message stored from Slack"
    );

    Ok(format!(
        "Steering message queued for session `{}`. It will be delivered on the next `ping`.",
        session.id
    ))
}

/// Store a steering message submitted via IPC (e.g., `intercom-ctl steer`).
///
/// Uses any active session since IPC commands are not channel-scoped.
/// Returns a success JSON value or an error string.
///
/// # Errors
///
/// Returns an `AppError` if session lookup or message insertion fails.
pub async fn store_from_ipc(text: &str, state: &Arc<AppState>) -> crate::Result<serde_json::Value> {
    if text.trim().is_empty() {
        return Err(crate::AppError::Config(
            "steering message text cannot be empty".into(),
        ));
    }

    let session_repo = SessionRepo::new(Arc::clone(&state.db));
    let sessions = session_repo
        .list_active()
        .await
        .map_err(|err| crate::AppError::Db(format!("failed to list sessions: {err}")))?;

    let session = sessions.into_iter().next().ok_or_else(|| {
        crate::AppError::Config("no active session to steer — start a session first".into())
    })?;

    let msg = SteeringMessage::new(
        session.id.clone(),
        None,
        text.to_owned(),
        SteeringSource::Ipc,
    );

    let steering_repo = SteeringRepo::new(Arc::clone(&state.db));
    steering_repo.insert(&msg).await?;

    info!(
        session_id = %session.id,
        "steering message stored from IPC"
    );

    Ok(serde_json::json!({
        "session_id": session.id,
        "queued": true,
    }))
}

/// Ingest a Slack app mention as a steering message.
///
/// Strips the bot mention prefix (e.g., `<@U1234>`) from the text before
/// storing, so operators can type `@intercom refocus on tests` naturally.
/// Non-empty text after stripping is stored; empty mentions are silently
/// ignored.
pub async fn ingest_app_mention(text: &str, channel_id: &str, state: &Arc<AppState>) {
    // Strip leading `<@UXXXXX>` mention token.
    let stripped = strip_mention(text).trim().to_owned();

    if stripped.is_empty() {
        info!(channel_id, "ignoring empty app mention");
        return;
    }

    match store_from_slack(&stripped, Some(channel_id), state).await {
        Ok(msg) => info!(channel_id, %msg, "app mention → steering message stored"),
        Err(err) => warn!(channel_id, %err, "failed to store app mention as steering message"),
    }
}

/// Strip `<@UXXXXX>` mention tokens from the start of a string.
///
/// Uses [`str::split_once`] to avoid byte-offset arithmetic that could
/// accidentally split a multi-byte UTF-8 sequence.
fn strip_mention(text: &str) -> &str {
    let trimmed = text.trim_start();
    if trimmed.starts_with("<@") {
        trimmed
            .split_once('>')
            .map_or(trimmed, |(_, rest)| rest.trim_start())
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::strip_mention;

    #[test]
    fn strip_mention_removes_at_prefix() {
        assert_eq!(
            strip_mention("<@U12345> refocus on tests"),
            "refocus on tests"
        );
    }

    #[test]
    fn strip_mention_leaves_plain_text_unchanged() {
        assert_eq!(strip_mention("refocus on tests"), "refocus on tests");
    }

    #[test]
    fn strip_mention_handles_empty_string() {
        assert_eq!(strip_mention(""), "");
    }
}
