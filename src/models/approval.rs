//! Approval request model for code proposal review.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Risk classification for a code proposal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    /// Low-risk change unlikely to cause issues.
    Low,
    /// High-risk change requiring careful review.
    High,
    /// Critical change affecting core functionality.
    Critical,
}

/// Lifecycle status for an approval request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    /// Awaiting operator decision.
    Pending,
    /// Operator accepted the proposal.
    Approved,
    /// Operator rejected the proposal.
    Rejected,
    /// Request timed out without response.
    Expired,
    /// Approved diff has been applied to the file system.
    Consumed,
    /// Request interrupted by server shutdown or crash.
    Interrupted,
}

/// A code proposal awaiting operator approval via Slack.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ApprovalRequest {
    /// Unique record identifier.
    pub id: String,
    /// Owning session identifier.
    pub session_id: String,
    /// Concise summary of the proposal.
    pub title: String,
    /// Contextual details about the proposed change.
    pub description: Option<String>,
    /// Unified diff or raw file content.
    pub diff_content: String,
    /// Target file path relative to workspace root.
    pub file_path: String,
    /// Risk classification.
    pub risk_level: RiskLevel,
    /// Current lifecycle status.
    pub status: ApprovalStatus,
    /// SHA-256 hash of the target file at proposal time.
    pub original_hash: String,
    /// Slack message timestamp for updates.
    pub slack_ts: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Timestamp when the approved diff was applied.
    pub consumed_at: Option<DateTime<Utc>>,
}

impl ApprovalRequest {
    /// Construct a new pending approval request.
    #[must_use]
    pub fn new(
        session_id: String,
        title: String,
        description: Option<String>,
        diff_content: String,
        file_path: String,
        risk_level: RiskLevel,
        original_hash: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            session_id,
            title,
            description,
            diff_content,
            file_path,
            risk_level,
            status: ApprovalStatus::Pending,
            original_hash,
            slack_ts: None,
            created_at: Utc::now(),
            consumed_at: None,
        }
    }
}
