//! Unit tests for policy loader (T116) and `CompiledWorkspacePolicy` (T047, T050).
//!
//! Validates `.intercom/settings.json` parsing, malformed file fallback
//! to deny-all, missing file handling, and regex pre-compilation.

use std::fs;
use std::path::Path;

use agent_intercom::models::approval::RiskLevel;
use agent_intercom::models::policy::{CompiledWorkspacePolicy, FilePatterns, WorkspacePolicy};
use agent_intercom::policy::loader::PolicyLoader;

/// Helper: write a policy JSON file under `workspace_root/.intercom/settings.json`.
fn write_policy(workspace_root: &Path, json: &str) {
    let dir = workspace_root.join(".intercom");
    fs::create_dir_all(&dir).expect("create .intercom dir");
    fs::write(dir.join("settings.json"), json).expect("write settings.json");
}

// ─── Valid policy file parsing ────────────────────────────────────────

#[test]
fn loads_valid_complete_policy() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_policy(
        dir.path(),
        r#"{
            "enabled": true,
            "auto_approve_commands": ["cargo test", "cargo clippy"],
            "tools": ["remote_log"],
            "file_patterns": {
                "write": ["src/**/*.rs"],
                "read": ["**/*"]
            },
            "risk_level_threshold": "high",
            "log_auto_approved": true,
            "summary_interval_seconds": 120
        }"#,
    );

    let policy = PolicyLoader::load(dir.path()).expect("should parse valid policy");

    assert!(policy.raw.enabled);
    assert_eq!(policy.raw.auto_approve_commands.len(), 2);
    assert_eq!(policy.raw.tools, vec!["remote_log".to_owned()]);
    assert_eq!(
        policy.raw.file_patterns.write,
        vec!["src/**/*.rs".to_owned()]
    );
    assert_eq!(policy.raw.file_patterns.read, vec!["**/*".to_owned()]);
    assert_eq!(policy.raw.risk_level_threshold, RiskLevel::High);
    assert!(policy.raw.log_auto_approved);
    assert_eq!(policy.raw.summary_interval_seconds, 120);
}

#[test]
fn loads_minimal_policy_with_defaults() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_policy(
        dir.path(),
        r#"{
            "enabled": true,
            "auto_approve_commands": []
        }"#,
    );

    let policy = PolicyLoader::load(dir.path()).expect("should parse minimal policy");

    assert!(policy.raw.enabled);
    assert!(policy.raw.auto_approve_commands.is_empty());
    assert!(policy.raw.tools.is_empty());
    assert_eq!(policy.raw.risk_level_threshold, RiskLevel::Low);
    assert!(!policy.raw.log_auto_approved);
    assert_eq!(policy.raw.summary_interval_seconds, 300);
}

// ─── Malformed file fallback to deny-all ──────────────────────────────

#[test]
fn malformed_json_returns_deny_all() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_policy(dir.path(), "{ this is not valid json }}}");

    let policy = PolicyLoader::load(dir.path()).expect("should return deny-all on malformed JSON");

    assert!(!policy.raw.enabled, "deny-all must have enabled=false");
    assert!(policy.raw.auto_approve_commands.is_empty());
    assert!(policy.raw.tools.is_empty());
}

#[test]
fn empty_file_returns_deny_all() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_policy(dir.path(), "");

    let policy = PolicyLoader::load(dir.path()).expect("should return deny-all on empty file");

    assert!(!policy.raw.enabled);
}

// ─── Commands preserved as-is (workspace-local concern) ───────────────

#[test]
fn commands_preserved_from_workspace_policy() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_policy(
        dir.path(),
        r#"{
            "enabled": true,
            "auto_approve_commands": ["cargo test", "rm -rf /", "cargo clippy"]
        }"#,
    );

    let policy = PolicyLoader::load(dir.path()).expect("should load with all commands preserved");

    assert_eq!(
        policy.raw.auto_approve_commands.len(),
        3,
        "all commands from workspace policy should be preserved"
    );
}

#[test]
fn commands_loaded_without_filtering() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_policy(
        dir.path(),
        r#"{
            "enabled": true,
            "auto_approve_commands": ["dangerous_command"]
        }"#,
    );

    let policy = PolicyLoader::load(dir.path()).expect("should load with commands preserved");

    assert_eq!(
        policy.raw.auto_approve_commands,
        vec!["dangerous_command".to_owned()],
        "workspace commands should be preserved as-is without global filtering"
    );
}

// ─── Missing policy file returns deny-all ─────────────────────────────

#[test]
fn missing_policy_file_returns_deny_all() {
    let dir = tempfile::tempdir().expect("tempdir");
    // No .intercom/settings.json created.

    let policy =
        PolicyLoader::load(dir.path()).expect("should return deny-all when file is missing");

    assert!(!policy.raw.enabled, "missing file must return deny-all");
    assert!(policy.raw.auto_approve_commands.is_empty());
}

#[test]
fn missing_agentrc_dir_returns_deny_all() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Even the .intercom directory doesn't exist.

    let policy =
        PolicyLoader::load(dir.path()).expect("should return deny-all when directory is missing");

    assert!(!policy.raw.enabled);
}

// ── US1: Policy directory constant assertion (T021) ──────────────────

/// T021: Policy directory uses `.intercom` (not `.agentrc`).
///
/// Verifies that `PolicyLoader::load` reads from `.intercom/settings.json`.
#[test]
fn policy_directory_is_dot_intercom() {
    let dir = tempfile::tempdir().expect("tempdir");

    // Write a policy under .intercom/ — the new directory name.
    write_policy(
        dir.path(),
        r#"{"enabled": true, "auto_approve_commands": ["cargo check"]}"#,
    );

    let policy = PolicyLoader::load(dir.path()).expect("load from .intercom");
    assert!(
        policy.raw.enabled,
        "should load policy from .intercom/settings.json"
    );

    // Verify it does NOT load from .agentrc/ by creating one there too.
    let old_dir = dir.path().join(".agentrc");
    std::fs::create_dir_all(&old_dir).expect("create .agentrc dir");
    std::fs::write(old_dir.join("settings.json"), r#"{"enabled": false}"#)
        .expect("write old settings");

    // Re-load: should still get enabled=true from .intercom, not false from .agentrc.
    let policy2 = PolicyLoader::load(dir.path()).expect("load from .intercom again");
    assert!(
        policy2.raw.enabled,
        "policy should come from .intercom, not .agentrc"
    );
}

// ─── CompiledWorkspacePolicy: regex pre-compilation (T047, T050) ─────

/// S074 — Patterns are compiled into a `RegexSet` at load time.
///
/// A policy with `N` command patterns should produce a `CompiledWorkspacePolicy`
/// with a `command_set` that matches correctly and `command_patterns` of length `N`.
#[test]
fn compiled_policy_has_expected_pattern_count() {
    let patterns: Vec<String> = (0..20).map(|i| format!("^cmd{i}$")).collect();
    let raw = WorkspacePolicy {
        enabled: true,
        auto_approve_commands: patterns,
        tools: Vec::new(),
        file_patterns: FilePatterns::default(),
        risk_level_threshold: RiskLevel::Low,
        log_auto_approved: false,
        summary_interval_seconds: 300,
    };
    let compiled = CompiledWorkspacePolicy::from_policy(raw);
    assert_eq!(
        compiled.command_patterns.len(),
        20,
        "all 20 valid patterns must be compiled"
    );
    assert!(compiled.command_set.is_match("cmd0"), "cmd0 must match");
    assert!(compiled.command_set.is_match("cmd19"), "cmd19 must match");
    assert!(
        !compiled.command_set.is_match("cmd20"),
        "cmd20 must not match"
    );
}

/// S077 — Creating a new `CompiledWorkspacePolicy` (simulating hot-reload) produces
/// an updated `RegexSet` that reflects the new pattern list.
#[test]
fn reloaded_policy_has_new_patterns() {
    let old = CompiledWorkspacePolicy::from_policy(WorkspacePolicy {
        enabled: true,
        auto_approve_commands: vec!["old_cmd".to_owned()],
        ..WorkspacePolicy::default()
    });
    let new = CompiledWorkspacePolicy::from_policy(WorkspacePolicy {
        enabled: true,
        auto_approve_commands: vec!["new_cmd".to_owned()],
        ..WorkspacePolicy::default()
    });

    assert!(
        old.command_set.is_match("old_cmd"),
        "old set matches old_cmd"
    );
    assert!(
        !old.command_set.is_match("new_cmd"),
        "old set must not match new_cmd"
    );
    assert!(
        new.command_set.is_match("new_cmd"),
        "new set matches new_cmd"
    );
    assert!(
        !new.command_set.is_match("old_cmd"),
        "new set must not match old_cmd"
    );
}

/// S078 — A policy with no command patterns yields an empty `RegexSet`; all commands
/// are denied at the evaluator level.
#[test]
fn empty_patterns_result_in_empty_regex_set() {
    let compiled = CompiledWorkspacePolicy::from_policy(WorkspacePolicy::default());
    assert_eq!(compiled.command_patterns.len(), 0, "no patterns compiled");
    assert!(
        !compiled.command_set.is_match("cargo test"),
        "empty set must not match any command"
    );
}

/// S076 — Invalid regex patterns are silently skipped; valid ones are compiled.
#[test]
fn invalid_regex_patterns_are_skipped_valid_ones_compiled() {
    let patterns = vec![
        "^valid_cmd$".to_owned(),
        "((UNCLOSED_GROUP".to_owned(),
        "^also_valid$".to_owned(),
    ];
    let raw = WorkspacePolicy {
        enabled: true,
        auto_approve_commands: patterns,
        ..WorkspacePolicy::default()
    };
    let compiled = CompiledWorkspacePolicy::from_policy(raw);

    assert_eq!(
        compiled.command_patterns.len(),
        2,
        "only the 2 valid patterns should be compiled"
    );
    assert!(
        compiled.command_set.is_match("valid_cmd"),
        "valid_cmd must match"
    );
    assert!(
        compiled.command_set.is_match("also_valid"),
        "also_valid must match"
    );
}

/// S079 — Patterns that could cause catastrophic backtracking in legacy regex
/// engines are handled safely by the `regex` crate (NFA/DFA-based).  The test
/// ensures no panic occurs and the result is deterministic.
#[test]
fn complex_regex_pattern_is_handled_safely() {
    // The `regex` crate rejects patterns that exceed its complexity limit,
    // so the pattern may be silently skipped.  Either 0 or 1 patterns are
    // compiled — but the call must not panic or block.
    let patterns = vec![
        r"^(a+)+$".to_owned(), // Classic catastrophic-backtracking pattern in PCRE
        r"^[a-zA-Z0-9_]+$".to_owned(), // Always-valid simple pattern
    ];
    let raw = WorkspacePolicy {
        enabled: true,
        auto_approve_commands: patterns,
        ..WorkspacePolicy::default()
    };
    let compiled = CompiledWorkspacePolicy::from_policy(raw);
    // At minimum the simple pattern compiles; complex one may or may not.
    assert!(
        !compiled.command_patterns.is_empty(),
        "at least 1 pattern must compile"
    );
    assert!(
        compiled.command_set.is_match("hello_world"),
        "simple pattern must match"
    );
}
