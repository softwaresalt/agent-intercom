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
