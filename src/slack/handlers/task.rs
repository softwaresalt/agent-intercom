//! Task inbox ingestion handler (T033).
//!
//! Provides shared logic for queuing work items from Slack slash commands
//! and IPC requests. Items are stored in the task inbox table and delivered
//! to the agent at cold-start via the `recover_state` (reboot) tool.

use std::sync::Arc;

use tracing::info;

use crate::mcp::handler::AppState;
use crate::models::inbox::{InboxSource, TaskInboxItem};
use crate::persistence::inbox_repo::InboxRepo;

/// Store a task inbox item from a Slack slash command.
///
/// Creates a new `TaskInboxItem` with `source = Slack` and inserts it into
/// the inbox. Returns an operator-visible confirmation string on success.
///
/// # Errors
///
/// Returns an `AppError` if the item cannot be inserted into the database.
pub async fn store_from_slack(
    text: &str,
    channel_id: Option<&str>,
    state: &Arc<AppState>,
) -> crate::Result<String> {
    if text.trim().is_empty() {
        return Err(crate::AppError::Config(
            "task message text cannot be empty".into(),
        ));
    }

    let item = TaskInboxItem::new(
        channel_id.map(str::to_owned),
        text.to_owned(),
        InboxSource::Slack,
    );

    let inbox_repo = InboxRepo::new(Arc::clone(&state.db));
    inbox_repo.insert(&item).await?;

    info!(
        task_id = %item.id,
        channel_id = ?channel_id,
        "task inbox item stored from Slack"
    );

    Ok(format!(
        "Task `{}` queued for next agent cold-start.",
        item.id
    ))
}

/// Store a task inbox item submitted via IPC (`intercom-ctl task`).
///
/// Creates a new `TaskInboxItem` with `source = Ipc` and inserts it into
/// the inbox. Returns a JSON value with `task_id` and `queued: true`.
///
/// # Errors
///
/// Returns an `AppError` if the item cannot be inserted into the database.
pub async fn store_from_ipc(text: &str, state: &Arc<AppState>) -> crate::Result<serde_json::Value> {
    if text.trim().is_empty() {
        return Err(crate::AppError::Config(
            "task message text cannot be empty".into(),
        ));
    }

    let item = TaskInboxItem::new(None, text.to_owned(), InboxSource::Ipc);

    let inbox_repo = InboxRepo::new(Arc::clone(&state.db));
    inbox_repo.insert(&item).await?;

    info!(task_id = %item.id, "task inbox item stored from IPC");

    Ok(serde_json::json!({
        "task_id": item.id,
        "queued": true,
    }))
}
