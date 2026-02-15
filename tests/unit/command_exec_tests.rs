//! Unit tests for command execution safety (T121).
//!
//! Validates that:
//! - Allowed commands pass execution pre-checks (FR-014).
//! - Disallowed commands are rejected.
//! - Path validation for `list-files` / `show-file` stays within
//!   the workspace root boundary (FR-006).

use std::collections::HashMap;

use monocoque_agent_rc::slack::commands::{
    file_extension_language, validate_command_alias, validate_listing_path,
};

// ─── Command alias validation (FR-014) ─────────────────────────────────

#[test]
fn allowed_command_passes_validation() {
    let mut allowlist = HashMap::new();
    allowlist.insert("test".to_owned(), "cargo test".to_owned());
    allowlist.insert("status".to_owned(), "git status".to_owned());

    let result = validate_command_alias("test", &allowlist);
    assert!(result.is_ok());
    assert_eq!(result.ok(), Some("cargo test".to_owned()));
}

#[test]
fn disallowed_command_is_rejected() {
    let allowlist = HashMap::new(); // Empty — nothing allowed.
    let result = validate_command_alias("rm-all", &allowlist);
    assert!(result.is_err());
}

#[test]
fn command_not_in_registry_is_rejected() {
    let mut allowlist = HashMap::new();
    allowlist.insert("test".to_owned(), "cargo test".to_owned());

    let result = validate_command_alias("deploy", &allowlist);
    assert!(result.is_err());
}

// ─── list-files / show-file path validation (FR-006) ───────────────────

#[test]
fn valid_relative_path_passes() {
    let workspace_root = std::env::temp_dir();
    // Create a temporary subdirectory to validate against.
    let sub = workspace_root.join("test_subdir_cmd_exec");
    std::fs::create_dir_all(&sub).ok();

    let result = validate_listing_path(Some("test_subdir_cmd_exec"), &workspace_root);
    assert!(result.is_ok());

    std::fs::remove_dir(&sub).ok();
}

#[test]
fn traversal_path_is_rejected() {
    let workspace_root = std::env::temp_dir();
    let result = validate_listing_path(Some("../../etc/passwd"), &workspace_root);
    assert!(result.is_err());
}

#[test]
fn none_path_defaults_to_workspace_root() {
    let workspace_root = std::env::temp_dir();
    let result = validate_listing_path(None, &workspace_root);
    assert!(result.is_ok());
    let resolved = result.ok();
    assert!(resolved.is_some());
    // The result should be the canonical workspace root itself.
    let canonical_root = workspace_root.canonicalize().ok();
    assert_eq!(resolved, canonical_root);
}

#[test]
fn absolute_path_within_workspace_passes() {
    let workspace_root = std::env::temp_dir();
    let sub = workspace_root.join("test_abs_cmd_exec");
    std::fs::create_dir_all(&sub).ok();

    // Pass the absolute path as a string.
    let result = validate_listing_path(Some(&sub.to_string_lossy()), &workspace_root);
    assert!(result.is_ok());

    std::fs::remove_dir(&sub).ok();
}

#[test]
fn absolute_path_outside_workspace_is_rejected() {
    // Create two sibling directories; one is the workspace, the other is outside.
    let parent = std::env::temp_dir().join("cmdexec_isolation");
    let workspace = parent.join("workspace_in");
    let outside = parent.join("outside_dir");
    std::fs::create_dir_all(&workspace).ok();
    std::fs::create_dir_all(&outside).ok();

    // Requesting the outside sibling from the workspace root should fail.
    let result = validate_listing_path(Some(&outside.to_string_lossy()), &workspace);
    assert!(result.is_err());

    std::fs::remove_dir_all(&parent).ok();
}

// ─── File extension to language mapping ────────────────────────────────

#[test]
fn known_extensions_map_correctly() {
    assert_eq!(file_extension_language("main.rs"), "rust");
    assert_eq!(file_extension_language("index.ts"), "typescript");
    assert_eq!(file_extension_language("style.css"), "css");
    assert_eq!(file_extension_language("Cargo.toml"), "toml");
    assert_eq!(file_extension_language("data.json"), "json");
    assert_eq!(file_extension_language("README.md"), "markdown");
}

#[test]
fn unknown_extension_defaults_to_text() {
    assert_eq!(file_extension_language("file.xyz"), "text");
    assert_eq!(file_extension_language("noext"), "text");
}
