//! Policy evaluator for workspace auto-approve rules (T062).
//!
//! Determines whether a given tool or command invocation can bypass the
//! remote approval gate based on the workspace [`WorkspacePolicy`] and
//! the global configuration. Global config always supersedes workspace
//! policy (FR-011).

use std::collections::HashMap;

use tracing::{info, info_span};

use crate::models::policy::WorkspacePolicy;

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
    /// 3. Match against `commands` (with global allowlist gate).
    /// 4. Match against `tools`.
    /// 5. Match against `file_patterns` (write/read globs).
    /// 6. If no rule matches, deny.
    #[must_use]
    pub fn check(
        tool_name: &str,
        context: &Option<AutoApproveContext>,
        policy: &WorkspacePolicy,
        global_commands: &HashMap<String, String>,
    ) -> AutoApproveResult {
        let _span = info_span!(
            "policy_evaluate",
            tool_name = %tool_name,
        )
        .entered();

        // ── 1. Disabled policy → deny all ────────────────────
        if !policy.enabled {
            return deny();
        }

        // ── 2. Risk level gate ───────────────────────────────
        if let Some(ref ctx) = context {
            if let Some(ref risk) = ctx.risk_level {
                if !risk_within_threshold(risk, &policy.risk_level_threshold) {
                    info!(
                        risk = %risk,
                        threshold = %policy.risk_level_threshold,
                        "risk exceeds threshold, denying auto-approve"
                    );
                    return deny();
                }
            }
        }

        // ── 3. Command matching (FR-011: must be in global allowlist) ─
        if policy.commands.contains(&tool_name.to_owned())
            && global_commands.contains_key(tool_name)
        {
            let rule = format!("command:{tool_name}");
            info!(matched_rule = %rule, "auto-approved via command rule");
            return approve(rule);
        }

        // ── 4. Tool matching ─────────────────────────────────
        if policy.tools.contains(&tool_name.to_owned()) {
            let rule = format!("tool:{tool_name}");
            info!(matched_rule = %rule, "auto-approved via tool rule");
            return approve(rule);
        }

        // ── 5. File pattern matching ─────────────────────────
        if let Some(ref ctx) = context {
            if let Some(ref file_path) = ctx.file_path {
                if let Some(rule) = match_file_patterns(tool_name, file_path, policy) {
                    info!(matched_rule = %rule, "auto-approved via file pattern rule");
                    return approve(rule);
                }
            }
        }

        // ── 6. No match → deny ──────────────────────────────
        deny()
    }
}

/// Risk levels ranked by severity (lower index = lower risk).
const RISK_LEVELS: &[&str] = &["low", "high", "critical"];

/// Check whether the request risk is within the policy threshold.
///
/// `critical` risk is never auto-approved regardless of threshold.
fn risk_within_threshold(request_risk: &str, threshold: &str) -> bool {
    let request_idx = RISK_LEVELS.iter().position(|&r| r == request_risk);
    let threshold_idx = RISK_LEVELS.iter().position(|&r| r == threshold);

    match (request_idx, threshold_idx) {
        // "critical" is never auto-approved.
        (Some(req), _) if RISK_LEVELS[req] == "critical" => false,
        (Some(req), Some(thr)) => req <= thr,
        // Unknown risk levels default to deny.
        _ => false,
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
