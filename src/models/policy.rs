//! Workspace auto-approve policy model.

use serde::Deserialize;

use crate::models::approval::RiskLevel;

/// File pattern rules for auto-approval matching.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct FilePatterns {
    /// Glob patterns for auto-approved file writes.
    #[serde(default)]
    pub write: Vec<String>,
    /// Glob patterns for auto-approved file reads.
    #[serde(default)]
    pub read: Vec<String>,
}

/// Workspace auto-approve configuration loaded from `.intercom/settings.json`.
///
/// In-memory only — not persisted to `SQLite`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct WorkspacePolicy {
    /// Master switch for auto-approve.
    #[serde(default)]
    pub enabled: bool,
    /// Shell command patterns that bypass approval (regex).
    ///
    /// Each entry is a regular expression matched against the full command
    /// line.  Plain command names (e.g. `"cargo test"`) still work because
    /// they match themselves literally.  Use anchors (`^…$`) and
    /// alternation to cover families of commands:
    ///
    /// ```json
    /// "auto_approve_commands": [
    ///   "^cargo (build|test|check|clippy|fmt)(\\s.*)?$"
    /// ]
    /// ```
    #[serde(default, alias = "commands")]
    pub auto_approve_commands: Vec<String>,
    /// MCP tool names that bypass approval.
    #[serde(default)]
    pub tools: Vec<String>,
    /// File pattern rules for writes and reads.
    #[serde(default)]
    pub file_patterns: FilePatterns,
    /// Maximum risk level for auto-approve.
    #[serde(default = "default_risk_threshold")]
    pub risk_level_threshold: RiskLevel,
    /// Whether to post auto-approved actions to Slack.
    #[serde(default)]
    pub log_auto_approved: bool,
    /// Interval for summary notifications (seconds).
    #[serde(default = "default_summary_interval")]
    pub summary_interval_seconds: u64,
}

fn default_risk_threshold() -> RiskLevel {
    RiskLevel::Low
}

fn default_summary_interval() -> u64 {
    300
}

impl Default for WorkspacePolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_approve_commands: Vec::new(),
            tools: Vec::new(),
            file_patterns: FilePatterns::default(),
            risk_level_threshold: default_risk_threshold(),
            log_auto_approved: false,
            summary_interval_seconds: default_summary_interval(),
        }
    }
}
