//! Policy evaluator for workspace auto-approve rules (T062).
//!
//! Determines whether a given tool or command invocation can bypass the
//! remote approval gate based on the workspace [`CompiledWorkspacePolicy`].
//! Auto-approve policy is entirely a workspace-local concern.

use regex::Regex;
use tracing::{info, info_span};

use crate::models::approval::RiskLevel;
use crate::models::policy::{CompiledWorkspacePolicy, WorkspacePolicy};

/// Additional metadata supplied by the agent for fine-grained evaluation.
#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct AutoApproveContext {
    /// Target file path (relative to workspace root).
    pub file_path: Option<String>,
    /// Risk level of the operation.
    pub risk_level: Option<String>,
}

/// Result of an auto-approve policy evaluation.
#[derive(Debug, Clone)]
pub struct AutoApproveResult {
    /// Whether the operation is auto-approved.
    pub auto_approved: bool,
    /// The rule key that matched, or `None` if denied.
    pub matched_rule: Option<String>,
}

/// Evaluates auto-approve policy rules against a tool invocation.
pub struct PolicyEvaluator;

impl PolicyEvaluator {
    /// Check whether `tool_name` is auto-approved under the given policy.
    ///
    /// Evaluation order:
    /// 1. If the policy is disabled, deny immediately.
    /// 2. Check risk level threshold — deny if exceeded.
    /// 3. Match against `auto_approve_commands`.
    /// 4. Match against `tools`.
    /// 5. Match against `file_patterns` (write/read globs).
    /// 6. If no rule matches, deny.
    #[must_use]
    pub fn check(
        tool_name: &str,
        context: &Option<AutoApproveContext>,
        policy: &CompiledWorkspacePolicy,
    ) -> AutoApproveResult {
        let _span = info_span!(
            "policy_evaluate",
            tool_name = %tool_name,
        )
        .entered();

        // ── 1. Disabled policy → deny all ────────────────────
        if !policy.raw.enabled {
            return deny();
        }

        // ── 2. Risk level gate ───────────────────────────────
        if let Some(ref ctx) = context {
            if let Some(ref risk) = ctx.risk_level {
                if !risk_within_threshold(risk, policy.raw.risk_level_threshold) {
                    info!(
                        risk = %risk,
                        threshold = ?policy.raw.risk_level_threshold,
                        "risk exceeds threshold, denying auto-approve"
                    );
                    return deny();
                }
            }
        }

        // ── 3. Command matching (regex) ──────────────────────
        if let Some(rule) = match_command_pattern(&policy.raw.auto_approve_commands, tool_name) {
            info!(matched_rule = %rule, "auto-approved via command rule");
            return approve(rule);
        }

        // ── 4. Tool matching ─────────────────────────────────
        if policy.raw.tools.contains(&tool_name.to_owned()) {
            let rule = format!("tool:{tool_name}");
            info!(matched_rule = %rule, "auto-approved via tool rule");
            return approve(rule);
        }

        // ── 5. File pattern matching ─────────────────────────
        if let Some(ref ctx) = context {
            if let Some(ref file_path) = ctx.file_path {
                if let Some(rule) = match_file_patterns(tool_name, file_path, &policy.raw) {
                    info!(matched_rule = %rule, "auto-approved via file pattern rule");
                    return approve(rule);
                }
            }
        }

        // ── 6. No match → deny ──────────────────────────────
        deny()
    }
}

/// Check whether the request risk is within the policy threshold.
///
/// `critical` risk is never auto-approved regardless of threshold.
fn risk_within_threshold(request_risk: &str, threshold: RiskLevel) -> bool {
    let request_level = match request_risk {
        "low" => RiskLevel::Low,
        "high" => RiskLevel::High,
        // critical and unknown levels are never auto-approved
        _ => return false,
    };

    risk_ordinal(request_level) <= risk_ordinal(threshold)
}

/// Map a `RiskLevel` to a numeric ordinal for comparison.
const fn risk_ordinal(level: RiskLevel) -> u8 {
    match level {
        RiskLevel::Low => 0,
        RiskLevel::High => 1,
        RiskLevel::Critical => 2,
    }
}

/// Attempt to match `file_path` against the policy's write/read glob patterns.
fn match_file_patterns(
    tool_name: &str,
    file_path: &str,
    policy: &WorkspacePolicy,
) -> Option<String> {
    // Determine which pattern set to check based on tool semantics.
    let (patterns, kind) = if tool_name.contains("write") || tool_name == "accept_diff" {
        (&policy.file_patterns.write, "write")
    } else if tool_name.contains("read") {
        (&policy.file_patterns.read, "read")
    } else {
        // Try write patterns first, then read patterns.
        if let Some(rule) = try_glob_match(&policy.file_patterns.write, file_path, "write") {
            return Some(rule);
        }
        return try_glob_match(&policy.file_patterns.read, file_path, "read");
    };

    try_glob_match(patterns, file_path, kind)
}

/// Try each glob pattern against the file path. Returns the first match.
fn try_glob_match(patterns: &[String], file_path: &str, kind: &str) -> Option<String> {
    for pattern in patterns {
        match glob::Pattern::new(pattern) {
            Ok(glob_pat) => {
                if glob_pat.matches(file_path) {
                    return Some(format!("file_pattern:{kind}:{pattern}"));
                }
            }
            Err(err) => {
                tracing::warn!(
                    pattern = %pattern,
                    %err,
                    "invalid glob pattern in workspace policy, skipping"
                );
            }
        }
    }
    None
}

/// Try each command pattern (regex) against the tool name / command line.
///
/// Returns the first matching rule as `command:<pattern>`.
fn match_command_pattern(patterns: &[String], command: &str) -> Option<String> {
    for pattern in patterns {
        match Regex::new(pattern) {
            Ok(re) => {
                if re.is_match(command) {
                    return Some(format!("command:{pattern}"));
                }
            }
            Err(err) => {
                tracing::warn!(
                    pattern = %pattern,
                    %err,
                    "invalid regex in workspace policy commands, skipping"
                );
            }
        }
    }
    None
}

/// Construct a deny result.
fn deny() -> AutoApproveResult {
    AutoApproveResult {
        auto_approved: false,
        matched_rule: None,
    }
}

/// Construct an approve result with the given rule.
fn approve(rule: String) -> AutoApproveResult {
    AutoApproveResult {
        auto_approved: true,
        matched_rule: Some(rule),
    }
}
