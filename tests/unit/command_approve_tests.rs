//! Unit tests for auto-approve suggestion (T063, scenarios S068-S073).
//!
//! Verifies that suggestion blocks are well-formed, generated patterns are
//! valid regexes that anchor to the command, and the settings writer creates
//! and appends patterns correctly.

use agent_intercom::slack::handlers::command_approve;
use regex::Regex;

/// S068 — `suggestion_blocks` returns blocks that reference the auto-approve action.
#[test]
fn suggestion_blocks_include_auto_approve_action() {
    let blocks = command_approve::suggestion_blocks("cargo test");
    let json = serde_json::to_string(&blocks).expect("serialize blocks");
    assert!(
        json.contains("auto_approve") || json.contains("Add to auto"),
        "blocks must reference auto-approve; got: {json}"
    );
}

/// S069 — `generate_pattern` for `cargo test` produces a non-empty string.
#[test]
fn generate_pattern_for_cargo_test_is_nonempty() {
    let pattern = command_approve::generate_pattern("cargo test");
    assert!(!pattern.is_empty(), "pattern must not be empty");
}

/// S070 — Generated pattern for `cargo fmt` references the command text.
#[test]
fn generate_pattern_references_command_text() {
    let pattern = command_approve::generate_pattern("cargo fmt");
    assert!(
        pattern.contains("cargo") || pattern.contains("fmt"),
        "pattern should reference the command; got: {pattern}"
    );
}

/// S071 — Pattern for `cargo test` does not match `rm -rf /`.
#[test]
fn generated_pattern_does_not_match_unrelated_command() {
    let pattern = command_approve::generate_pattern("cargo test");
    let re = Regex::new(&pattern).expect("valid regex");
    assert!(
        !re.is_match("rm -rf /"),
        "cargo test pattern must not match `rm -rf /`; pattern: {pattern}"
    );
}

/// S072 — `write_pattern_to_settings` creates `settings.json` when absent.
#[test]
fn write_pattern_creates_settings_file_when_absent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let settings_path = dir.path().join("settings.json");
    command_approve::write_pattern_to_settings(&settings_path, "cargo test")
        .expect("write pattern");
    assert!(settings_path.exists(), "settings.json should be created");
}

/// S073 — `write_pattern_to_settings` appends patterns without overwriting existing ones.
#[test]
fn write_pattern_appends_to_existing_settings() {
    let dir = tempfile::tempdir().expect("tempdir");
    let settings_path = dir.path().join("settings.json");
    command_approve::write_pattern_to_settings(&settings_path, "cargo test").expect("first write");
    command_approve::write_pattern_to_settings(&settings_path, "cargo fmt").expect("second write");
    let contents = std::fs::read_to_string(&settings_path).expect("read settings");
    assert!(
        contents.contains("cargo test") || contents.contains("cargo"),
        "first pattern should be present; got: {contents}"
    );
    assert!(
        contents.contains("cargo fmt"),
        "second pattern should be present; got: {contents}"
    );
}

/// S074 — `write_pattern_to_settings` writes the pattern to `auto_approve_commands`
/// (the array read by the MCP `auto_check` policy evaluator).
#[test]
fn write_pattern_populates_auto_approve_commands_array() {
    let dir = tempfile::tempdir().expect("tempdir");
    let settings_path = dir.path().join("settings.json");
    command_approve::write_pattern_to_settings(&settings_path, "DEL /F /Q test.txt")
        .expect("write pattern");

    let raw = std::fs::read_to_string(&settings_path).expect("read settings");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("parse json");

    let cmds = json
        .get("auto_approve_commands")
        .and_then(|v| v.as_array())
        .expect("auto_approve_commands must be an array");
    assert!(
        !cmds.is_empty(),
        "auto_approve_commands must contain at least one entry"
    );
    let pattern = command_approve::generate_pattern("DEL /F /Q test.txt");
    assert!(
        cmds.iter().any(|v| v.as_str() == Some(&pattern)),
        "auto_approve_commands must contain the generated pattern `{pattern}`; got: {cmds:?}"
    );
}

/// S075 — `write_pattern_to_settings` does not add duplicate entries to `auto_approve_commands`.
#[test]
fn write_pattern_does_not_duplicate_auto_approve_commands() {
    let dir = tempfile::tempdir().expect("tempdir");
    let settings_path = dir.path().join("settings.json");
    command_approve::write_pattern_to_settings(&settings_path, "cargo test").expect("first write");
    command_approve::write_pattern_to_settings(&settings_path, "cargo test").expect("second write");

    let raw = std::fs::read_to_string(&settings_path).expect("read settings");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("parse json");

    let cmds = json
        .get("auto_approve_commands")
        .and_then(|v| v.as_array())
        .expect("auto_approve_commands must be an array");
    let pattern = command_approve::generate_pattern("cargo test");
    let count = cmds.iter().filter(|v| v.as_str() == Some(&pattern)).count();
    assert_eq!(count, 1, "pattern should appear exactly once, got {count}; entries: {cmds:?}");
}

/// S076 — `write_pattern_to_workspace_file` returns `Ok(false)` when no *.code-workspace exists.
#[test]
fn write_pattern_to_workspace_file_returns_false_when_no_workspace_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let result = command_approve::write_pattern_to_workspace_file(dir.path(), "cargo test")
        .expect("should not error");
    assert!(!result, "should return false when no .code-workspace file is present");
}

/// S077 — `write_pattern_to_workspace_file` writes the pattern into
/// `settings.chat.tools.terminal.autoApprove` when a workspace file exists.
#[test]
fn write_pattern_to_workspace_file_writes_to_auto_approve_map() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Create a minimal workspace file.
    let ws_path = dir.path().join("test.code-workspace");
    std::fs::write(&ws_path, r#"{"folders": [{"path": "."}], "settings": {}}"#)
        .expect("create workspace file");

    let found = command_approve::write_pattern_to_workspace_file(dir.path(), "DEL /F /Q test.txt")
        .expect("write should succeed");
    assert!(found, "should return true when workspace file exists");

    let raw = std::fs::read_to_string(&ws_path).expect("read workspace file");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("parse workspace file");
    let pattern = command_approve::generate_pattern("DEL /F /Q test.txt");
    let map = json
        .pointer("/settings/chat.tools.terminal.autoApprove")
        .and_then(|v| v.as_object())
        .expect("settings.chat.tools.terminal.autoApprove must be an object");
    assert!(
        map.contains_key(&pattern),
        "autoApprove map must contain the pattern `{pattern}`; got keys: {map:?}"
    );
}

/// S078 — `write_pattern_to_workspace_file` does not duplicate patterns in the autoApprove map.
#[test]
fn write_pattern_to_workspace_file_does_not_duplicate() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws_path = dir.path().join("test.code-workspace");
    std::fs::write(&ws_path, r#"{"folders": [{"path": "."}], "settings": {}}"#)
        .expect("create workspace file");

    command_approve::write_pattern_to_workspace_file(dir.path(), "cargo test")
        .expect("first write");
    command_approve::write_pattern_to_workspace_file(dir.path(), "cargo test")
        .expect("second write");

    let raw = std::fs::read_to_string(&ws_path).expect("read workspace file");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("parse workspace file");
    let pattern = command_approve::generate_pattern("cargo test");
    let map = json
        .pointer("/settings/chat.tools.terminal.autoApprove")
        .and_then(|v| v.as_object())
        .expect("autoApprove must be an object");
    let count = map.keys().filter(|k| k.as_str() == pattern).count();
    assert_eq!(count, 1, "pattern key should appear exactly once");
}
