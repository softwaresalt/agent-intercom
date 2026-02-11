//! Stall alert model for agent inactivity detection.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::progress::ProgressItem;

/// Lifecycle status for a stall alert.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StallAlertStatus {
    /// Alert created, awaiting operator response.
    Pending,
    /// Agent was nudged by operator or auto-nudge.
    Nudged,
    /// Agent resumed activity on its own.
    SelfRecovered,
    /// Max nudge retries exceeded, escalated to channel.
    Escalated,
    /// Alert dismissed by operator or system.
    Dismissed,
}

/// A watchdog notification triggered by detected agent inactivity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct StallAlert {
    /// Unique record identifier.
    pub id: String,
    /// Owning session identifier.
    pub session_id: String,
    /// Name of last tool called before stall.
    pub last_tool: Option<String>,
    /// Timestamp of last detected MCP activity.
    pub last_activity_at: DateTime<Utc>,
    /// Elapsed idle time when alert was created.
    pub idle_seconds: i64,
    /// Number of nudge attempts for this alert.
    pub nudge_count: u32,
    /// Current lifecycle status.
    pub status: StallAlertStatus,
    /// Custom nudge message from operator.
    pub nudge_message: Option<String>,
    /// Session's progress snapshot at alert time.
    pub progress_snapshot: Option<Vec<ProgressItem>>,
    /// Slack message timestamp for updates.
    pub slack_ts: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

impl StallAlert {
    /// Construct a new pending stall alert.
    #[must_use]
    pub fn new(
        session_id: String,
        last_tool: Option<String>,
        last_activity_at: DateTime<Utc>,
        idle_seconds: i64,
        progress_snapshot: Option<Vec<ProgressItem>>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            session_id,
            last_tool,
            last_activity_at,
            idle_seconds,
            nudge_count: 0,
            status: StallAlertStatus::Pending,
            nudge_message: None,
            progress_snapshot,
            slack_ts: None,
            created_at: Utc::now(),
        }
    }
}
