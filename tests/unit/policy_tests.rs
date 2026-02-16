//! Unit tests for policy loader (T116).
//!
//! Validates `.monocoque/settings.json` parsing, malformed file fallback
//! to deny-all, global allowlist enforcement (FR-011), and missing file
//! handling.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use monocoque_agent_rc::models::approval::RiskLevel;
use monocoque_agent_rc::policy::loader::PolicyLoader;

/// Helper: write a policy JSON file under `workspace_root/.monocoque/settings.json`.
fn write_policy(workspace_root: &Path, json: &str) {
    let dir = workspace_root.join(".monocoque");
    fs::create_dir_all(&dir).expect("create .monocoque dir");
    fs::write(dir.join("settings.json"), json).expect("write settings.json");
}

/// Helper: build a minimal global commands allowlist.
fn allowlist(commands: &[&str]) -> HashMap<String, String> {
    commands
        .iter()
        .map(|c| ((*c).to_owned(), (*c).to_owned()))
        .collect()
}

// ─── Valid policy file parsing ────────────────────────────────────────

#[test]
fn loads_valid_complete_policy() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_policy(
        dir.path(),
        r#"{
            "enabled": true,
            "commands": ["cargo test", "cargo clippy"],
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

    let global_commands = allowlist(&["cargo test", "cargo clippy"]);
    let policy =
        PolicyLoader::load(dir.path(), &global_commands).expect("should parse valid policy");

    assert!(policy.enabled);
    assert_eq!(policy.commands.len(), 2);
    assert_eq!(policy.tools, vec!["remote_log".to_owned()]);
    assert_eq!(policy.file_patterns.write, vec!["src/**/*.rs".to_owned()]);
    assert_eq!(policy.file_patterns.read, vec!["**/*".to_owned()]);
    assert_eq!(policy.risk_level_threshold, RiskLevel::High);
    assert!(policy.log_auto_approved);
    assert_eq!(policy.summary_interval_seconds, 120);
}

#[test]
fn loads_minimal_policy_with_defaults() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_policy(
        dir.path(),
        r#"{
            "enabled": true,
            "commands": []
        }"#,
    );

    let global_commands = HashMap::new();
    let policy =
        PolicyLoader::load(dir.path(), &global_commands).expect("should parse minimal policy");

    assert!(policy.enabled);
    assert!(policy.commands.is_empty());
    assert!(policy.tools.is_empty());
    assert_eq!(policy.risk_level_threshold, RiskLevel::Low);
    assert!(!policy.log_auto_approved);
    assert_eq!(policy.summary_interval_seconds, 300);
}

// ─── Malformed file fallback to deny-all ──────────────────────────────

#[test]
fn malformed_json_returns_deny_all() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_policy(dir.path(), "{ this is not valid json }}}");

    let global_commands = HashMap::new();
    let policy = PolicyLoader::load(dir.path(), &global_commands)
        .expect("should return deny-all on malformed JSON");

    assert!(!policy.enabled, "deny-all must have enabled=false");
    assert!(policy.commands.is_empty());
    assert!(policy.tools.is_empty());
}

#[test]
fn empty_file_returns_deny_all() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_policy(dir.path(), "");

    let global_commands = HashMap::new();
    let policy = PolicyLoader::load(dir.path(), &global_commands)
        .expect("should return deny-all on empty file");

    assert!(!policy.enabled);
}

// ─── Commands not in global allowlist rejected (FR-011) ───────────────

#[test]
fn strips_commands_not_in_global_allowlist() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_policy(
        dir.path(),
        r#"{
            "enabled": true,
            "commands": ["cargo test", "rm -rf /", "cargo clippy"]
        }"#,
    );

    // Only "cargo test" is globally allowed.
    let global_commands = allowlist(&["cargo test"]);
    let policy = PolicyLoader::load(dir.path(), &global_commands)
        .expect("should load with filtered commands");

    assert_eq!(
        policy.commands,
        vec!["cargo test".to_owned()],
        "commands not in the global allowlist must be stripped"
    );
}

#[test]
fn all_commands_rejected_when_none_in_allowlist() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_policy(
        dir.path(),
        r#"{
            "enabled": true,
            "commands": ["dangerous_command"]
        }"#,
    );

    let global_commands = HashMap::new();
    let policy =
        PolicyLoader::load(dir.path(), &global_commands).expect("should load with empty commands");

    assert!(
        policy.commands.is_empty(),
        "all commands should be stripped when none match the global allowlist"
    );
}

// ─── Missing policy file returns deny-all ─────────────────────────────

#[test]
fn missing_policy_file_returns_deny_all() {
    let dir = tempfile::tempdir().expect("tempdir");
    // No .monocoque/settings.json created.

    let global_commands = HashMap::new();
    let policy = PolicyLoader::load(dir.path(), &global_commands)
        .expect("should return deny-all when file is missing");

    assert!(!policy.enabled, "missing file must return deny-all");
    assert!(policy.commands.is_empty());
}

#[test]
fn missing_monocoque_dir_returns_deny_all() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Even the .monocoque directory doesn't exist.

    let global_commands = HashMap::new();
    let policy = PolicyLoader::load(dir.path(), &global_commands)
        .expect("should return deny-all when directory is missing");

    assert!(!policy.enabled);
}
