//! Workspace auto-approve policy model.

use regex::RegexSet;
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
    /// "chat.tools.terminal.autoApprove": [
    ///   "^cargo (build|test|check|clippy|fmt)(\\s.*)?$"
    /// ]
    /// ```
    ///
    /// Accepted JSON keys (in order of preference):
    /// - `chat.tools.terminal.autoApprove` — shared key with VS Code workspace
    /// - `auto_approve_commands` — legacy key (still accepted)
    /// - `commands` — short alias
    #[serde(default, alias = "auto_approve_commands", alias = "commands")]
    #[serde(rename = "chat.tools.terminal.autoApprove")]
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

/// Pre-compiled form of [`WorkspacePolicy`] with command regex patterns compiled
/// into a [`RegexSet`] for efficient matching.
///
/// Created by [`crate::policy::loader::PolicyLoader::load`] and cached in the
/// shared `PolicyCache` for reuse across requests.
#[derive(Debug, Clone)]
pub struct CompiledWorkspacePolicy {
    /// Original policy data (used for non-command evaluations).
    pub raw: WorkspacePolicy,
    /// Pre-compiled command pattern set.
    ///
    /// Each index in the set corresponds to the same index in
    /// [`Self::command_patterns`], enabling matched-rule reporting.
    pub command_set: RegexSet,
    /// Original pattern strings, parallel to [`Self::command_set`].
    pub command_patterns: Vec<String>,
}

impl CompiledWorkspacePolicy {
    /// Construct from a [`WorkspacePolicy`], compiling command patterns.
    ///
    /// Invalid patterns are silently skipped with a tracing warning.
    #[must_use]
    pub fn from_policy(raw: WorkspacePolicy) -> Self {
        let valid_patterns: Vec<String> = raw
            .auto_approve_commands
            .iter()
            .filter(|p| {
                let ok = regex::Regex::new(p).is_ok();
                if !ok {
                    tracing::warn!(pattern = %p, "invalid regex in policy commands, skipping");
                }
                ok
            })
            .cloned()
            .collect();

        let command_set = RegexSet::new(&valid_patterns).unwrap_or_else(|_| RegexSet::empty());

        Self {
            raw,
            command_set,
            command_patterns: valid_patterns,
        }
    }

    /// Return a deny-all compiled policy with no patterns.
    #[must_use]
    pub fn deny_all() -> Self {
        Self {
            raw: WorkspacePolicy::default(),
            command_set: RegexSet::empty(),
            command_patterns: Vec::new(),
        }
    }
}
