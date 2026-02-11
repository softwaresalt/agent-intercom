//! Session model and lifecycle helpers.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Lifecycle status for an agent session.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Session created but not yet activated.
    Created,
    /// Session actively running.
    Active,
    /// Session paused by operator.
    Paused,
    /// Session terminated explicitly.
    Terminated,
    /// Session interrupted by crash or shutdown.
    Interrupted,
}

/// Operational routing mode for the session.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    /// All interactions routed through Slack.
    Remote,
    /// All interactions routed through local IPC.
    Local,
    /// Both Slack and IPC are active; first response wins.
    Hybrid,
}

/// Session domain entity persisted in `SurrealDB`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct Session {
    /// Unique record identifier.
    pub id: String,
    /// Owning Slack user ID; immutable after creation.
    pub owner_user_id: String,
    /// Current lifecycle status.
    pub status: SessionStatus,
    /// Optional initial prompt/instruction.
    pub prompt: Option<String>,
    /// Operational mode for routing.
    pub mode: SessionMode,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last activity timestamp.
    pub updated_at: DateTime<Utc>,
    /// Most recent tool called.
    pub last_tool: Option<String>,
    /// Consecutive nudge attempts for current stall.
    pub nudge_count: u32,
    /// Whether stall detection is currently paused.
    pub stall_paused: bool,
}

impl Session {
    /// Construct a new session with defaults and generated identifier.
    #[must_use]
    pub fn new(owner_user_id: String, prompt: Option<String>, mode: SessionMode) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            owner_user_id,
            status: SessionStatus::Created,
            prompt,
            mode,
            created_at: now,
            updated_at: now,
            last_tool: None,
            nudge_count: 0,
            stall_paused: false,
        }
    }

    /// Determine whether a lifecycle transition is permitted.
    #[must_use]
    pub fn can_transition_to(&self, next: SessionStatus) -> bool {
        matches!(
            (self.status, next),
            (
                SessionStatus::Created | SessionStatus::Paused,
                SessionStatus::Active
            ) | (
                SessionStatus::Active,
                SessionStatus::Paused | SessionStatus::Terminated | SessionStatus::Interrupted
            ) | (
                SessionStatus::Paused,
                SessionStatus::Terminated | SessionStatus::Interrupted
            )
        )
    }
}
