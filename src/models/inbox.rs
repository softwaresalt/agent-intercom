//! Task inbox item model for cold-start work queuing.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Ingestion source for a task inbox item.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InboxSource {
    /// Item submitted via Slack slash command.
    Slack,
    /// Item submitted via IPC (`intercom-ctl task`).
    Ipc,
}

/// A work item queued for delivery to an agent at cold-start via `reboot`.
///
/// Task inbox items allow operators to queue work when no agent session is
/// running. Items are delivered at session start and scoped to the Slack
/// channel that created them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskInboxItem {
    /// Unique record identifier (UUID v4 prefixed `task:`).
    pub id: String,
    /// Slack channel scope for delivery matching (optional).
    pub channel_id: Option<String>,
    /// Work item text.
    pub message: String,
    /// Ingestion path for this item.
    pub source: InboxSource,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Whether this item has been delivered via `reboot`.
    pub consumed: bool,
}

impl TaskInboxItem {
    /// Construct a new task inbox item with a generated identifier.
    #[must_use]
    pub fn new(channel_id: Option<String>, message: String, source: InboxSource) -> Self {
        Self {
            id: format!("task:{}", Uuid::new_v4()),
            channel_id,
            message,
            source,
            created_at: Utc::now(),
            consumed: false,
        }
    }
}
