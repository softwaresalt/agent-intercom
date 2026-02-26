//! Unit tests for policy evaluator (T117).
//!
//! Validates command matching, tool matching, file pattern glob matching,
//! and `risk_level_threshold` enforcement.

use agent_intercom::models::approval::RiskLevel;
use agent_intercom::models::policy::{CompiledWorkspacePolicy, FilePatterns, WorkspacePolicy};
use agent_intercom::policy::evaluator::{AutoApproveContext, PolicyEvaluator};

/// Helper to build a policy with the given overrides applied to defaults.
fn policy(enabled: bool, commands: &[&str], tools: &[&str]) -> CompiledWorkspacePolicy {
    CompiledWorkspacePolicy::from_policy(WorkspacePolicy {
        enabled,
        auto_approve_commands: commands.iter().map(|s| (*s).to_owned()).collect(),
        tools: tools.iter().map(|s| (*s).to_owned()).collect(),
        file_patterns: FilePatterns::default(),
        risk_level_threshold: RiskLevel::Low,
        log_auto_approved: false,
        summary_interval_seconds: 300,
    })
}

// ─── Command matching ─────────────────────────────────────────────────

#[test]
fn command_in_policy_is_auto_approved() {
    let wp = policy(true, &["^cargo test$"], &[]);

    let result = PolicyEvaluator::check("cargo test", &None, &wp);
    assert!(result.auto_approved);
    assert_eq!(result.matched_rule.as_deref(), Some("command:^cargo test$"));
}

#[test]
fn command_not_in_policy_is_denied() {
    let wp = policy(true, &["^cargo test$"], &[]);

    let result = PolicyEvaluator::check("cargo clippy", &None, &wp);
    assert!(!result.auto_approved);
    assert!(result.matched_rule.is_none());
}

// ─── Regex command matching ───────────────────────────────────────────

#[test]
fn regex_pattern_matches_cargo_subcommands() {
    let wp = policy(
        true,
        &[r"^cargo (build|test|check|clippy|fmt)(\s[^;|&`]*)?$"],
        &[],
    );

    // Bare cargo subcommand
    assert!(PolicyEvaluator::check("cargo test", &None, &wp).auto_approved);
    // Cargo subcommand with arguments
    assert!(PolicyEvaluator::check("cargo test --release", &None, &wp).auto_approved);
    // Cargo subcommand with additional flags
    assert!(PolicyEvaluator::check("cargo clippy -- -D warnings", &None, &wp).auto_approved);
    // Different subcommand
    assert!(PolicyEvaluator::check("cargo fmt", &None, &wp).auto_approved);
}

#[test]
fn regex_pattern_rejects_disallowed_commands() {
    let wp = policy(
        true,
        &[r"^cargo (build|test|check|clippy|fmt)(\s[^;|&`]*)?$"],
        &[],
    );

    // Not in the allowed subcommands
    assert!(!PolicyEvaluator::check("cargo install malware", &None, &wp).auto_approved);
    // Chained with semicolons (blocked by character class)
    assert!(!PolicyEvaluator::check("cargo test; rm -rf /", &None, &wp).auto_approved);
    // Completely different command
    assert!(!PolicyEvaluator::check("rm -rf /", &None, &wp).auto_approved);
}

#[test]
fn regex_pattern_matches_piped_output() {
    let wp = policy(
        true,
        &[
            r"^cargo (build|test|check|clippy|fmt)(\s[^;|&`]*)?(\s*(>|>>|2>&1|\|\s*(Out-File|Set-Content|Out-String))\s*[^;|&`]*)*$",
        ],
        &[],
    );

    assert!(
        PolicyEvaluator::check(
            r"cargo test 2>&1 | Out-File logs\test-results.txt",
            &None,
            &wp,
        )
        .auto_approved
    );
    assert!(PolicyEvaluator::check("cargo check > logs/check.txt 2>&1", &None, &wp).auto_approved);
}

#[test]
fn regex_pattern_matches_git_with_args() {
    let wp = policy(
        true,
        &[r"^git (status|add|commit|diff|log|push)(\s[^;|&`]*)?$"],
        &[],
    );

    assert!(PolicyEvaluator::check("git status", &None, &wp).auto_approved);
    assert!(PolicyEvaluator::check("git commit -m \"fix: patch\"", &None, &wp).auto_approved);
    assert!(PolicyEvaluator::check("git add src/main.rs", &None, &wp).auto_approved);
    assert!(!PolicyEvaluator::check("git rebase main", &None, &wp).auto_approved);
}

#[test]
fn invalid_regex_is_skipped() {
    // Malformed regex (unmatched parenthesis) should be skipped, not panic
    let wp = policy(true, &["^cargo (build|test", "^git status$"], &[]);

    // The invalid regex is skipped, but the second pattern still matches
    assert!(PolicyEvaluator::check("git status", &None, &wp).auto_approved);
    // The invalid first pattern doesn't match (it's skipped)
    assert!(!PolicyEvaluator::check("cargo build", &None, &wp).auto_approved);
}

#[test]
fn multiple_regex_patterns_first_match_wins() {
    let wp = policy(true, &[r"^cargo test$", r"^cargo (build|test|check)"], &[]);

    let result = PolicyEvaluator::check("cargo test", &None, &wp);
    assert!(result.auto_approved);
    assert_eq!(
        result.matched_rule.as_deref(),
        Some("command:^cargo test$"),
        "should match the first pattern"
    );
}

// ─── Tool matching ────────────────────────────────────────────────────

#[test]
fn tool_in_policy_is_auto_approved() {
    let wp = policy(true, &[], &["remote_log"]);

    let result = PolicyEvaluator::check("remote_log", &None, &wp);
    assert!(result.auto_approved);
    assert_eq!(result.matched_rule.as_deref(), Some("tool:remote_log"));
}

#[test]
fn tool_not_in_policy_is_denied() {
    let wp = policy(true, &[], &["remote_log"]);

    let result = PolicyEvaluator::check("ask_approval", &None, &wp);
    assert!(!result.auto_approved);
}

// ─── File pattern glob matching ───────────────────────────────────────

#[test]
fn write_file_pattern_matches() {
    let wp = CompiledWorkspacePolicy::from_policy(WorkspacePolicy {
        enabled: true,
        file_patterns: FilePatterns {
            write: vec!["src/**/*.rs".to_owned()],
            read: vec![],
        },
        ..WorkspacePolicy::default()
    });
    let ctx = Some(AutoApproveContext {
        file_path: Some("src/main.rs".to_owned()),
        risk_level: None,
    });

    let result = PolicyEvaluator::check("write_file", &ctx, &wp);
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
    let wp = CompiledWorkspacePolicy::from_policy(WorkspacePolicy {
        enabled: true,
        file_patterns: FilePatterns {
            write: vec!["tests/**/*.rs".to_owned()],
            read: vec![],
        },
        ..WorkspacePolicy::default()
    });
    let ctx = Some(AutoApproveContext {
        file_path: Some("src/main.rs".to_owned()),
        risk_level: None,
    });

    let result = PolicyEvaluator::check("write_file", &ctx, &wp);
    assert!(!result.auto_approved);
}

#[test]
fn read_file_pattern_matches() {
    let wp = CompiledWorkspacePolicy::from_policy(WorkspacePolicy {
        enabled: true,
        file_patterns: FilePatterns {
            write: vec![],
            read: vec!["**/*".to_owned()],
        },
        ..WorkspacePolicy::default()
    });
    let ctx = Some(AutoApproveContext {
        file_path: Some("any/file.txt".to_owned()),
        risk_level: None,
    });

    let result = PolicyEvaluator::check("read_file", &ctx, &wp);
    assert!(result.auto_approved);
}

// ─── Risk level threshold enforcement ─────────────────────────────────

#[test]
fn risk_exceeding_threshold_denied() {
    let wp = CompiledWorkspacePolicy::from_policy(WorkspacePolicy {
        enabled: true,
        tools: vec!["ask_approval".to_owned()],
        risk_level_threshold: RiskLevel::Low,
        ..WorkspacePolicy::default()
    });
    let ctx = Some(AutoApproveContext {
        file_path: None,
        risk_level: Some("high".to_owned()),
    });

    let result = PolicyEvaluator::check("ask_approval", &ctx, &wp);
    assert!(
        !result.auto_approved,
        "high risk should be denied when threshold is low"
    );
}

#[test]
fn risk_within_threshold_approved() {
    let wp = CompiledWorkspacePolicy::from_policy(WorkspacePolicy {
        enabled: true,
        tools: vec!["ask_approval".to_owned()],
        risk_level_threshold: RiskLevel::High,
        ..WorkspacePolicy::default()
    });
    let ctx = Some(AutoApproveContext {
        file_path: None,
        risk_level: Some("low".to_owned()),
    });

    let result = PolicyEvaluator::check("ask_approval", &ctx, &wp);
    assert!(result.auto_approved);
}

#[test]
fn critical_risk_always_denied() {
    let wp = CompiledWorkspacePolicy::from_policy(WorkspacePolicy {
        enabled: true,
        tools: vec!["ask_approval".to_owned()],
        risk_level_threshold: RiskLevel::High,
        ..WorkspacePolicy::default()
    });
    let ctx = Some(AutoApproveContext {
        file_path: None,
        risk_level: Some("critical".to_owned()),
    });

    let result = PolicyEvaluator::check("ask_approval", &ctx, &wp);
    assert!(
        !result.auto_approved,
        "critical risk should always be denied"
    );
}

// ─── Disabled policy ──────────────────────────────────────────────────

#[test]
fn disabled_policy_denies_everything() {
    let wp = policy(false, &["cargo test"], &["remote_log"]);

    let result = PolicyEvaluator::check("cargo test", &None, &wp);
    assert!(
        !result.auto_approved,
        "disabled policy must deny all operations"
    );
}

// ─── No context edge case ─────────────────────────────────────────────

#[test]
fn no_context_still_matches_commands_and_tools() {
    let wp = policy(true, &["cargo test"], &["remote_log"]);

    let cmd_result = PolicyEvaluator::check("cargo test", &None, &wp);
    assert!(cmd_result.auto_approved);

    let tool_result = PolicyEvaluator::check("remote_log", &None, &wp);
    assert!(tool_result.auto_approved);
}
