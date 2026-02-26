//! Unit tests for policy loader (T116).
//!
//! Validates `.intercom/settings.json` parsing, malformed file fallback
//! to deny-all, and missing file handling.

use std::fs;
use std::path::Path;

use agent_intercom::models::approval::RiskLevel;
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
