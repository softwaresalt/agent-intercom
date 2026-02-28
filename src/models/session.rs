//! Session model and lifecycle helpers.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::progress::ProgressItem;

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

/// Agent connectivity state — separate from session lifecycle status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectivityStatus {
    /// Agent is actively communicating (stream messages or tool calls arriving).
    Online,
    /// Agent process is alive but no recent activity.
    Offline,
    /// Stall detector has flagged this session for inactivity.
    Stalled,
}

/// Protocol used by the agent for this session.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolMode {
    /// Agent communicates via the Model Context Protocol (MCP).
    Mcp,
    /// Agent communicates via the Agent Client Protocol (ACP) over stdio.
    Acp,
}

/// Session domain entity persisted in `SQLite`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct Session {
    /// Unique record identifier.
    pub id: String,
    /// Owning Slack user ID; immutable after creation.
    pub owner_user_id: String,
    /// Absolute path to the workspace directory for this session.
    pub workspace_root: String,
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
    pub nudge_count: i64,
    /// Whether stall detection is currently paused.
    pub stall_paused: bool,
    /// Timestamp when the session was terminated.
    pub terminated_at: Option<DateTime<Utc>>,
    /// Last-reported progress snapshot from the agent.
    pub progress_snapshot: Option<Vec<ProgressItem>>,
    /// Agent communication protocol for this session. Immutable after creation.
    pub protocol_mode: ProtocolMode,
    /// Slack channel ID where this session's messages are posted.
    pub channel_id: Option<String>,
    /// Slack thread timestamp of the session's root message.
    ///
    /// `None` until the first message is posted. Immutable once set.
    pub thread_ts: Option<String>,
    /// Agent connectivity state — separate from lifecycle status.
    pub connectivity_status: ConnectivityStatus,
    /// Timestamp of last agent activity for stall detection and recovery.
    pub last_activity_at: Option<DateTime<Utc>>,
    /// Session ID of the predecessor session if this is a restart, otherwise `None`.
    pub restart_of: Option<String>,
}

impl Session {
    /// Construct a new session with defaults and generated identifier.
    #[must_use]
    pub fn new(
        owner_user_id: String,
        workspace_root: String,
        prompt: Option<String>,
        mode: SessionMode,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            owner_user_id,
            workspace_root,
            status: SessionStatus::Created,
            prompt,
            mode,
            created_at: now,
            updated_at: now,
            last_tool: None,
            nudge_count: 0,
            stall_paused: false,
            terminated_at: None,
            progress_snapshot: None,
            protocol_mode: ProtocolMode::Mcp,
            channel_id: None,
            thread_ts: None,
            connectivity_status: ConnectivityStatus::Online,
            last_activity_at: None,
            restart_of: None,
        }
    }

    /// Determine whether a lifecycle transition is permitted.
    #[must_use]
    pub fn can_transition_to(&self, next: SessionStatus) -> bool {
        matches!(
            (self.status, next),
            (
                SessionStatus::Created | SessionStatus::Paused | SessionStatus::Interrupted,
                SessionStatus::Active
            ) | (
                SessionStatus::Active,
                SessionStatus::Paused | SessionStatus::Interrupted | SessionStatus::Terminated
            ) | (
                SessionStatus::Paused,
                SessionStatus::Terminated | SessionStatus::Interrupted
            )
        )
    }
}
