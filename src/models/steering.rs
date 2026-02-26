//! Steering message model for the operator-to-agent communication queue.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Ingestion source for a steering message.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SteeringSource {
    /// Message submitted via Slack app mention or slash command.
    Slack,
    /// Message submitted via IPC (`intercom-ctl steer`).
    Ipc,
}

/// An operator-to-agent message queued for delivery via `ping`.
///
/// Steering messages allow operators to proactively communicate with a
/// running agent session without waiting for the agent to reach an
/// interaction boundary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SteeringMessage {
    /// Unique record identifier (UUID v4 prefixed `steer:`).
    pub id: String,
    /// Target session identifier.
    pub session_id: String,
    /// Slack channel the message originated from, if applicable.
    pub channel_id: Option<String>,
    /// Free-text instruction from the operator.
    pub message: String,
    /// Ingestion path for this message.
    pub source: SteeringSource,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Whether this message has been delivered via `ping`.
    pub consumed: bool,
}

impl SteeringMessage {
    /// Construct a new steering message with a generated identifier.
    #[must_use]
    pub fn new(
        session_id: String,
        channel_id: Option<String>,
        message: String,
        source: SteeringSource,
    ) -> Self {
        Self {
            id: format!("steer:{}", Uuid::new_v4()),
            session_id,
            channel_id,
            message,
            source,
            created_at: Utc::now(),
            consumed: false,
        }
    }
}
