//! Unit tests for slash command utilities (T121).
//!
//! Validates that:
//! - Path validation for `list-files` / `show-file` stays within
//!   the workspace root boundary (FR-006).

use agent_intercom::slack::commands::{file_extension_language, validate_listing_path};

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
