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

/// Deserialize `chat.tools.terminal.autoApprove` from either:
/// - A **map** `{ "pattern": true }` or `{ "pattern": { "approve": true, ... } }`
///   — the format used by VS Code (`.code-workspace`, `.vscode/settings.json`)
/// - An **array** `["pattern1", "pattern2"]`
///   — the legacy format (backward compat)
///
/// In both cases the result is a `Vec<String>` of pattern strings (map keys or
/// array elements).
fn deserialize_auto_approve_commands<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{SeqAccess, MapAccess, Visitor};
    use std::fmt;

    struct AutoApproveVisitor;

    impl<'de> Visitor<'de> for AutoApproveVisitor {
        type Value = Vec<String>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a map or array of command patterns")
        }

        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Vec<String>, M::Error> {
            let mut patterns = Vec::new();
            while let Some(key) = map.next_key::<String>()? {
                // Consume (and discard) the value — we only need keys as patterns.
                map.next_value::<serde_json::Value>()?;
                patterns.push(key);
            }
            Ok(patterns)
        }

        fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Vec<String>, S::Error> {
            let mut patterns = Vec::new();
            while let Some(elem) = seq.next_element::<String>()? {
                patterns.push(elem);
            }
            Ok(patterns)
        }
    }

    deserializer.deserialize_any(AutoApproveVisitor)
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
    /// Accepts the same `chat.tools.terminal.autoApprove` key used by VS Code
    /// in `.code-workspace` and `.vscode/settings.json`, so a single setting
    /// is shared across all three files.
    ///
    /// The value may be either:
    /// - A **map** `{ "pattern": true }` or `{ "pattern": { "approve": true, "matchCommandLine": true } }`
    ///   (VS Code / `.code-workspace` native format — preferred)
    /// - An **array** `["pattern", ...]` (legacy format — still accepted)
    ///
    /// Accepted JSON keys (aliases accepted for backward compat):
    /// - `chat.tools.terminal.autoApprove` — primary, shared with VS Code
    /// - `auto_approve_commands` — legacy alias
    /// - `commands` — short alias
    #[serde(default, deserialize_with = "deserialize_auto_approve_commands")]
    #[serde(rename = "chat.tools.terminal.autoApprove", alias = "auto_approve_commands", alias = "commands")]
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
