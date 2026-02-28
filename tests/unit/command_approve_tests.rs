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

/// S074 — `write_pattern_to_settings` writes the pattern to
/// `chat.tools.terminal.autoApprove` as a map — matching the format used by
/// VS Code in `.code-workspace` and `.vscode/settings.json`.
#[test]
fn write_pattern_populates_auto_approve_commands_map() {
    let dir = tempfile::tempdir().expect("tempdir");
    let settings_path = dir.path().join("settings.json");
    command_approve::write_pattern_to_settings(&settings_path, "DEL /F /Q test.txt")
        .expect("write pattern");

    let raw = std::fs::read_to_string(&settings_path).expect("read settings");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("parse json");

    let map = json
        .get("chat.tools.terminal.autoApprove")
        .and_then(|v| v.as_object())
        .expect("chat.tools.terminal.autoApprove must be an object");
    assert!(
        !map.is_empty(),
        "chat.tools.terminal.autoApprove must contain at least one entry"
    );
    let pattern = command_approve::generate_pattern("DEL /F /Q test.txt");
    assert!(
        map.contains_key(&pattern),
        "chat.tools.terminal.autoApprove must contain the generated pattern `{pattern}` as a key; got: {map:?}"
    );
}

/// S075 — `write_pattern_to_settings` does not add duplicate entries to `chat.tools.terminal.autoApprove`.
#[test]
fn write_pattern_does_not_duplicate_auto_approve_commands() {
    let dir = tempfile::tempdir().expect("tempdir");
    let settings_path = dir.path().join("settings.json");
    command_approve::write_pattern_to_settings(&settings_path, "cargo test").expect("first write");
    command_approve::write_pattern_to_settings(&settings_path, "cargo test").expect("second write");

    let raw = std::fs::read_to_string(&settings_path).expect("read settings");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("parse json");

    let map = json
        .get("chat.tools.terminal.autoApprove")
        .and_then(|v| v.as_object())
        .expect("chat.tools.terminal.autoApprove must be an object");
    let pattern = command_approve::generate_pattern("cargo test");
    let count = map.keys().filter(|k| **k == pattern).count();
    assert_eq!(
        count, 1,
        "pattern should appear exactly once as a map key, got {count}; keys: {map:?}"
    );
}

/// S076 — `write_pattern_to_workspace_file` returns `Ok(false)` when no *.code-workspace exists.
#[test]
fn write_pattern_to_workspace_file_returns_false_when_no_workspace_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let result = command_approve::write_pattern_to_workspace_file(dir.path(), "cargo test")
        .expect("should not error");
    assert!(
        !result,
        "should return false when no .code-workspace file is present"
    );
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

/// S082 — `generate_pattern` for a simple OS command anchors to the base
/// command only, not to specific flags or file paths.
#[test]
fn generate_pattern_simple_command_anchors_to_base_command_only() {
    let pattern = command_approve::generate_pattern("DEL /F /Q tests\\fixtures\\hitl-scratch.txt");
    let re = Regex::new(&pattern).expect("valid regex");

    // Must match the base command with different arguments.
    assert!(
        re.is_match("DEL /F /Q other-file.txt"),
        "should match DEL with other args"
    );
    assert!(
        re.is_match("DEL some-other-file.txt"),
        "should match DEL with any file"
    );
    assert!(re.is_match("DEL"), "should match bare DEL");

    // Must NOT match a completely different command.
    assert!(!re.is_match("rmdir /S /Q folder"), "should not match rmdir");
    assert!(!re.is_match("cargo test"), "should not match cargo test");

    // Pattern must NOT contain the specific path from the original command.
    assert!(
        !pattern.contains("fixtures"),
        "pattern should not contain the filename path; got: {pattern}"
    );
}

/// S083 — `generate_pattern` for a multi-level command (`cargo`) captures the
/// base command AND the subcommand, but wildcards flags and file arguments.
#[test]
fn generate_pattern_multilevel_command_captures_base_and_subcommand() {
    let pattern = command_approve::generate_pattern("cargo test --release src/main.rs");
    let re = Regex::new(&pattern).expect("valid regex");

    // Must match the same base+subcommand with different flags.
    assert!(re.is_match("cargo test"), "should match bare 'cargo test'");
    assert!(
        re.is_match("cargo test --release"),
        "should match with --release flag"
    );
    assert!(
        re.is_match("cargo test src/other.rs"),
        "should match with other file"
    );

    // Must NOT match a different cargo subcommand.
    assert!(
        !re.is_match("cargo build"),
        "should not match 'cargo build'"
    );
    assert!(
        !re.is_match("cargo clippy"),
        "should not match 'cargo clippy'"
    );
    assert!(!re.is_match("cargo"), "should not match bare 'cargo'");

    // Pattern should contain "cargo test" anchor.
    assert!(
        pattern.contains("cargo test"),
        "pattern must contain 'cargo test' anchor; got: {pattern}"
    );
}

/// S084 — `generate_pattern` for `git add src/main.rs` anchors to `git add`.
#[test]
fn generate_pattern_git_add_anchors_to_git_add() {
    let pattern = command_approve::generate_pattern("git add src/main.rs");
    let re = Regex::new(&pattern).expect("valid regex");

    assert!(
        re.is_match("git add src/main.rs"),
        "should match original command"
    );
    assert!(re.is_match("git add ."), "should match `git add .`");
    assert!(
        !re.is_match("git commit -m 'msg'"),
        "should not match git commit"
    );
    assert!(!re.is_match("git push"), "should not match git push");
}

/// S085 — `generate_pattern` for `rmdir /S /Q backup` anchors to `rmdir` only.
#[test]
fn generate_pattern_rmdir_anchors_to_rmdir_only() {
    let pattern = command_approve::generate_pattern("rmdir /S /Q backup");
    let re = Regex::new(&pattern).expect("valid regex");

    assert!(re.is_match("rmdir /S /Q backup"), "should match original");
    assert!(
        re.is_match("rmdir /S /Q other-dir"),
        "should match other dirs"
    );
    assert!(re.is_match("rmdir"), "should match bare rmdir");
    assert!(!re.is_match("DEL /F /Q file"), "should not match DEL");

    // Must not bake in the specific path.
    assert!(
        !pattern.contains("backup"),
        "pattern must not contain the specific dir name; got: {pattern}"
    );
}
/// S079 — `write_pattern_to_vscode_settings` returns `Ok(false)` when `.vscode/settings.json`
/// does not exist.
#[test]
fn write_pattern_to_vscode_settings_returns_false_when_absent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let result = command_approve::write_pattern_to_vscode_settings(dir.path(), "cargo test")
        .expect("should not error");
    assert!(
        !result,
        "should return false when .vscode/settings.json is absent"
    );
}

/// S080 — `write_pattern_to_vscode_settings` writes the pattern into the top-level
/// `chat.tools.terminal.autoApprove` map when the file exists.
#[test]
fn write_pattern_to_vscode_settings_writes_to_auto_approve_map() {
    let dir = tempfile::tempdir().expect("tempdir");
    let vscode_dir = dir.path().join(".vscode");
    std::fs::create_dir_all(&vscode_dir).expect("create .vscode dir");
    let settings_path = vscode_dir.join("settings.json");
    std::fs::write(&settings_path, r#"{"chat.tools.terminal.autoApprove": {}}"#)
        .expect("create settings.json");

    let found = command_approve::write_pattern_to_vscode_settings(dir.path(), "DEL /F /Q test.txt")
        .expect("write should succeed");
    assert!(
        found,
        "should return true when .vscode/settings.json exists"
    );

    let raw = std::fs::read_to_string(&settings_path).expect("read settings.json");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("parse settings.json");
    let pattern = command_approve::generate_pattern("DEL /F /Q test.txt");
    let map = json
        .get("chat.tools.terminal.autoApprove")
        .and_then(|v| v.as_object())
        .expect("chat.tools.terminal.autoApprove must be an object");
    assert!(
        map.contains_key(&pattern),
        "autoApprove map must contain the pattern `{pattern}`; got keys: {map:?}"
    );
}

/// S081 — `write_pattern_to_vscode_settings` does not duplicate patterns.
#[test]
fn write_pattern_to_vscode_settings_does_not_duplicate() {
    let dir = tempfile::tempdir().expect("tempdir");
    let vscode_dir = dir.path().join(".vscode");
    std::fs::create_dir_all(&vscode_dir).expect("create .vscode dir");
    let settings_path = vscode_dir.join("settings.json");
    std::fs::write(&settings_path, r#"{"chat.tools.terminal.autoApprove": {}}"#)
        .expect("create settings.json");

    command_approve::write_pattern_to_vscode_settings(dir.path(), "cargo test")
        .expect("first write");
    command_approve::write_pattern_to_vscode_settings(dir.path(), "cargo test")
        .expect("second write");

    let raw = std::fs::read_to_string(&settings_path).expect("read settings.json");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("parse settings.json");
    let pattern = command_approve::generate_pattern("cargo test");
    let map = json
        .get("chat.tools.terminal.autoApprove")
        .and_then(|v| v.as_object())
        .expect("autoApprove must be an object");
    let count = map.keys().filter(|k| k.as_str() == pattern).count();
    assert_eq!(count, 1, "pattern key should appear exactly once");
}
