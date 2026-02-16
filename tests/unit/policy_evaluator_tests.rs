//! Unit tests for policy evaluator (T117).
//!
//! Validates command matching, tool matching, file pattern glob matching,
//! `risk_level_threshold` enforcement, and global config superseding
//! workspace config.

use std::collections::HashMap;

use monocoque_agent_rc::models::approval::RiskLevel;
use monocoque_agent_rc::models::policy::{FilePatterns, WorkspacePolicy};
use monocoque_agent_rc::policy::evaluator::{AutoApproveContext, PolicyEvaluator};

/// Helper to build a policy with the given overrides applied to defaults.
fn policy(enabled: bool, commands: &[&str], tools: &[&str]) -> WorkspacePolicy {
    WorkspacePolicy {
        enabled,
        commands: commands.iter().map(|s| (*s).to_owned()).collect(),
        tools: tools.iter().map(|s| (*s).to_owned()).collect(),
        file_patterns: FilePatterns::default(),
        risk_level_threshold: RiskLevel::Low,
        log_auto_approved: false,
        summary_interval_seconds: 300,
    }
}

/// Helper to build a global commands allowlist.
fn allowlist(commands: &[&str]) -> HashMap<String, String> {
    commands
        .iter()
        .map(|c| ((*c).to_owned(), (*c).to_owned()))
        .collect()
}

// ─── Command matching ─────────────────────────────────────────────────

#[test]
fn command_in_policy_is_auto_approved() {
    let wp = policy(true, &["cargo test"], &[]);
    let global = allowlist(&["cargo test"]);

    let result = PolicyEvaluator::check("cargo test", &None, &wp, &global);
    assert!(result.auto_approved);
    assert_eq!(result.matched_rule.as_deref(), Some("command:cargo test"));
}

#[test]
fn command_not_in_policy_is_denied() {
    let wp = policy(true, &["cargo test"], &[]);
    let global = allowlist(&["cargo test", "cargo clippy"]);

    let result = PolicyEvaluator::check("cargo clippy", &None, &wp, &global);
    assert!(!result.auto_approved);
    assert!(result.matched_rule.is_none());
}

#[test]
fn command_not_in_global_allowlist_denied_even_if_in_policy() {
    let wp = policy(true, &["rm -rf /"], &[]);
    let global = HashMap::new(); // Empty global allowlist.

    let result = PolicyEvaluator::check("rm -rf /", &None, &wp, &global);
    assert!(
        !result.auto_approved,
        "global config must supersede workspace config (FR-011)"
    );
}

// ─── Tool matching ────────────────────────────────────────────────────

#[test]
fn tool_in_policy_is_auto_approved() {
    let wp = policy(true, &[], &["remote_log"]);
    let global = HashMap::new();

    let result = PolicyEvaluator::check("remote_log", &None, &wp, &global);
    assert!(result.auto_approved);
    assert_eq!(result.matched_rule.as_deref(), Some("tool:remote_log"));
}

#[test]
fn tool_not_in_policy_is_denied() {
    let wp = policy(true, &[], &["remote_log"]);
    let global = HashMap::new();

    let result = PolicyEvaluator::check("ask_approval", &None, &wp, &global);
    assert!(!result.auto_approved);
}

// ─── File pattern glob matching ───────────────────────────────────────

#[test]
fn write_file_pattern_matches() {
    let wp = WorkspacePolicy {
        enabled: true,
        file_patterns: FilePatterns {
            write: vec!["src/**/*.rs".to_owned()],
            read: vec![],
        },
        ..WorkspacePolicy::default()
    };
    let global = HashMap::new();
    let ctx = Some(AutoApproveContext {
        file_path: Some("src/main.rs".to_owned()),
        risk_level: None,
    });

    let result = PolicyEvaluator::check("write_file", &ctx, &wp, &global);
    assert!(result.auto_approved);
    assert!(
        result
            .matched_rule
            .as_ref()
            .is_some_and(|r| r.starts_with("file_pattern:")),
        "expected file_pattern rule, got {:?}",
        result.matched_rule
    );
}

#[test]
fn write_file_pattern_no_match() {
    let wp = WorkspacePolicy {
        enabled: true,
        file_patterns: FilePatterns {
            write: vec!["tests/**/*.rs".to_owned()],
            read: vec![],
        },
        ..WorkspacePolicy::default()
    };
    let global = HashMap::new();
    let ctx = Some(AutoApproveContext {
        file_path: Some("src/main.rs".to_owned()),
        risk_level: None,
    });

    let result = PolicyEvaluator::check("write_file", &ctx, &wp, &global);
    assert!(!result.auto_approved);
}

#[test]
fn read_file_pattern_matches() {
    let wp = WorkspacePolicy {
        enabled: true,
        file_patterns: FilePatterns {
            write: vec![],
            read: vec!["**/*".to_owned()],
        },
        ..WorkspacePolicy::default()
    };
    let global = HashMap::new();
    let ctx = Some(AutoApproveContext {
        file_path: Some("any/file.txt".to_owned()),
        risk_level: None,
    });

    let result = PolicyEvaluator::check("read_file", &ctx, &wp, &global);
    assert!(result.auto_approved);
}

// ─── Risk level threshold enforcement ─────────────────────────────────

#[test]
fn risk_exceeding_threshold_denied() {
    let wp = WorkspacePolicy {
        enabled: true,
        tools: vec!["ask_approval".to_owned()],
        risk_level_threshold: RiskLevel::Low,
        ..WorkspacePolicy::default()
    };
    let global = HashMap::new();
    let ctx = Some(AutoApproveContext {
        file_path: None,
        risk_level: Some("high".to_owned()),
    });

    let result = PolicyEvaluator::check("ask_approval", &ctx, &wp, &global);
    assert!(
        !result.auto_approved,
        "high risk should be denied when threshold is low"
    );
}

#[test]
fn risk_within_threshold_approved() {
    let wp = WorkspacePolicy {
        enabled: true,
        tools: vec!["ask_approval".to_owned()],
        risk_level_threshold: RiskLevel::High,
        ..WorkspacePolicy::default()
    };
    let global = HashMap::new();
    let ctx = Some(AutoApproveContext {
        file_path: None,
        risk_level: Some("low".to_owned()),
    });

    let result = PolicyEvaluator::check("ask_approval", &ctx, &wp, &global);
    assert!(result.auto_approved);
}

#[test]
fn critical_risk_always_denied() {
    let wp = WorkspacePolicy {
        enabled: true,
        tools: vec!["ask_approval".to_owned()],
        risk_level_threshold: RiskLevel::High,
        ..WorkspacePolicy::default()
    };
    let global = HashMap::new();
    let ctx = Some(AutoApproveContext {
        file_path: None,
        risk_level: Some("critical".to_owned()),
    });

    let result = PolicyEvaluator::check("ask_approval", &ctx, &wp, &global);
    assert!(
        !result.auto_approved,
        "critical risk should always be denied"
    );
}

// ─── Disabled policy ──────────────────────────────────────────────────

#[test]
fn disabled_policy_denies_everything() {
    let wp = policy(false, &["cargo test"], &["remote_log"]);
    let global = allowlist(&["cargo test"]);

    let result = PolicyEvaluator::check("cargo test", &None, &wp, &global);
    assert!(
        !result.auto_approved,
        "disabled policy must deny all operations"
    );
}

// ─── No context edge case ─────────────────────────────────────────────

#[test]
fn no_context_still_matches_commands_and_tools() {
    let wp = policy(true, &["cargo test"], &["remote_log"]);
    let global = allowlist(&["cargo test"]);

    let cmd_result = PolicyEvaluator::check("cargo test", &None, &wp, &global);
    assert!(cmd_result.auto_approved);

    let tool_result = PolicyEvaluator::check("remote_log", &None, &wp, &global);
    assert!(tool_result.auto_approved);
}
